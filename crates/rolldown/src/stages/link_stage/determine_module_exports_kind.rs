use rolldown_common::{
  EcmaModuleAstUsage, ExportsKind, ImportKind, ImportRecordIdx, ImportRecordMeta, Module,
  OutputFormat, WrapKind,
};

use crate::utils::external_import_interop::import_record_needs_interop;

use super::LinkStage;

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn determine_module_exports_kind(&mut self) {
    // Iterate by index so that we can release the iterator borrow on `module_table`
    // before mutating an importee's `exports_kind` via `as_normal_mut`. Reads and
    // writes interleave in the same order as the original closure walk, preserving
    // the "earlier importer's promotion is observed by later importers" semantics.
    let importer_indices: Vec<_> = self
      .module_table
      .modules
      .iter_enumerated()
      .filter_map(|(idx, m)| matches!(m, Module::Normal(_)).then_some(idx))
      .collect();

    for importer_idx in importer_indices {
      let n_records = match &self.module_table[importer_idx] {
        Module::Normal(m) => m.import_records.len(),
        Module::External(_) => continue,
      };

      for rec_pos in 0..n_records {
        let (kind, importee_idx) = {
          let Module::Normal(m) = &self.module_table[importer_idx] else { continue };
          let rec = &m.import_records[ImportRecordIdx::from_usize(rec_pos)];
          let Some(importee_idx) = rec.resolved_module else { continue };
          (rec.kind, importee_idx)
        };
        let (importee_kind, has_lazy) = match &self.module_table[importee_idx] {
          Module::Normal(m) => (m.exports_kind, m.meta.has_lazy_export()),
          Module::External(_) => continue,
        };
        match kind {
          ImportKind::Import => {
            if matches!(importee_kind, ExportsKind::None) && !has_lazy {
              // `import`ing a module with `ExportsKind::None` promotes it to `ExportsKind::Esm`.
              if let Some(m) = self.module_table[importee_idx].as_normal_mut() {
                m.exports_kind = ExportsKind::Esm;
              }
            }
          }
          ImportKind::Require => match importee_kind {
            ExportsKind::Esm => {
              self.metas[importee_idx].sync_wrap_kind(WrapKind::Esm);
            }
            ExportsKind::CommonJs => {
              self.metas[importee_idx].sync_wrap_kind(WrapKind::Cjs);
            }
            ExportsKind::None => {
              self.metas[importee_idx].sync_wrap_kind(WrapKind::Cjs);
              // A `require`'d module with `ExportsKind::None` is promoted to `ExportsKind::CommonJs`.
              if let Some(m) = self.module_table[importee_idx].as_normal_mut() {
                m.exports_kind = ExportsKind::CommonJs;
              }
            }
          },
          ImportKind::DynamicImport => {
            if self.options.code_splitting.is_disabled() {
              // When code splitting is disabled (e.g. iife/umd/cjs output), `import()` behaves
              // like a `require()` that returns a promise, so the imported module must be wrapped.
              match importee_kind {
                ExportsKind::Esm => {
                  self.metas[importee_idx].sync_wrap_kind(WrapKind::Esm);
                }
                ExportsKind::CommonJs => {
                  self.metas[importee_idx].sync_wrap_kind(WrapKind::Cjs);
                }
                ExportsKind::None => {
                  self.metas[importee_idx].sync_wrap_kind(WrapKind::Cjs);
                  // A dynamically-imported module with `ExportsKind::None` is promoted to `ExportsKind::CommonJs`
                  // since we wrap it as CJS.
                  if let Some(m) = self.module_table[importee_idx].as_normal_mut() {
                    m.exports_kind = ExportsKind::CommonJs;
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
      }

      let Module::Normal(importer) = &self.module_table[importer_idx] else { continue };
      let is_entry = self.entries.contains_key(&importer.idx);
      if matches!(importer.exports_kind, ExportsKind::CommonJs)
        && (!is_entry
          || matches!(self.options.format, OutputFormat::Esm)
          || (matches!(self.options.format, OutputFormat::Iife | OutputFormat::Umd)
            && importer.ast_usage.intersects(EcmaModuleAstUsage::ModuleOrExports)))
      {
        self.metas[importer.idx].sync_wrap_kind(WrapKind::Cjs);
      }
    }
  }

  /// Builds the `safely_merge_cjs_ns_map` which groups ESM imports of the same CommonJS module.
  ///
  /// This optimization allows multiple ESM imports of the same CommonJS module to share
  /// a single namespace binding, reducing code size.
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn determine_safely_merge_cjs_ns(&mut self) {
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
