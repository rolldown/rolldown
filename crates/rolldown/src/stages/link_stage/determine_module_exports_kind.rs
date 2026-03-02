use rolldown_common::{ExportsKind, Module, OutputFormat};

use super::LinkStage;
use super::oxc_conversions::{from_oxc_exports_kind, from_oxc_wrap_kind};

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn determine_module_exports_kind(&mut self) {
    let config = oxc_module_graph::ExportsKindConfig {
      dynamic_imports_as_require: self.options.code_splitting.is_disabled(),
      wrap_cjs_entries: matches!(self.options.format, OutputFormat::Esm),
    };

    let result =
      oxc_module_graph::determine_module_exports_kind(&self.link_kernel.graph, &config);

    // Sync to Rolldown data BEFORE apply() consumes the result.
    for (oxc_idx, exports_kind) in &result.exports_kind_updates {
      let rd_idx = rolldown_common::ModuleIdx::from_usize(oxc_idx.index());
      if let Module::Normal(m) = &mut self.module_table[rd_idx] {
        m.exports_kind = from_oxc_exports_kind(*exports_kind);
      }
    }
    for (oxc_idx, wrap_kind) in &result.wrap_kind_updates {
      let rd_idx = rolldown_common::ModuleIdx::from_usize(oxc_idx.index());
      self.metas[rd_idx].sync_wrap_kind(from_oxc_wrap_kind(*wrap_kind));
    }

    // Apply to graph (writes exports_kind + wrap_kind on graph modules).
    result.apply(&mut self.link_kernel.graph);
  }

  /// Builds the `safely_merge_cjs_ns_map` which groups ESM imports of the same CommonJS module.
  ///
  /// This optimization allows multiple ESM imports of the same CommonJS module to share
  /// a single namespace binding, reducing code size.
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn determine_safely_merge_cjs_ns(&mut self) {
    use rolldown_common::{ImportKind, ImportRecordMeta};

    use crate::utils::external_import_interop::import_record_needs_interop;

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
