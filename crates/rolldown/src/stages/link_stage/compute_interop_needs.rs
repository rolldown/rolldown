use rolldown_common::{ImportKind, Module, WrapKind};

use super::LinkStage;
use crate::utils::external_import_interop::import_record_needs_interop;

impl LinkStage<'_> {
  /// Computes which wrapped CJS modules need interop globally.
  ///
  /// For each wrapped CJS module, checks if ANY import from ANY module uses
  /// default or namespace imports. If so, ALL imports of that module should
  /// use __toESM to ensure the default property exists.
  ///
  /// This fixes the issue where a CJS module imported with only named imports
  /// in one module doesn't use __toESM, but the same module imported with
  /// default import in another module does use __toESM, causing inconsistency.
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn compute_interop_needs(&mut self) {
    // For each module, check its imports
    for importer in self.module_table.modules.iter() {
      let Some(importer) = importer.as_normal() else {
        continue;
      };

      // Check each import record
      for (rec_id, rec) in importer.import_records.iter_enumerated() {
        // Only care about static imports
        if rec.kind != ImportKind::Import {
          continue;
        }

        // Get the importee module
        let Module::Normal(importee) = &self.module_table.modules[rec.resolved_module] else {
          continue;
        };

        // Only care about wrapped CJS modules
        let importee_linking_info = &self.metas[importee.idx];
        if importee_linking_info.wrap_kind() != WrapKind::Cjs {
          continue;
        }

        // Check if this import record needs interop
        if import_record_needs_interop(importer, rec_id) {
          // Mark the importee module as needing interop
          self.metas[importee.idx].needs_interop = true;
        }
      }
    }
  }
}
