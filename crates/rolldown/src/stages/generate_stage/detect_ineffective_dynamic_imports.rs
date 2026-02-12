use rolldown_error::{BuildDiagnostic, EventKindSwitcher};
use rolldown_std_utils::PathExt as _;
use sugar_path::SugarPath as _;

use crate::chunk_graph::ChunkGraph;

use super::GenerateStage;

impl GenerateStage<'_> {
  pub fn detect_ineffective_dynamic_imports(&mut self, chunk_graph: &ChunkGraph) {
    if self.options.code_splitting.is_disabled()
      || !self.options.checks.contains(EventKindSwitcher::IneffectiveDynamicImport)
    {
      return;
    }

    for chunk in chunk_graph.chunk_table.iter() {
      let pre_rendered_chunk =
        chunk.pre_rendered_chunk.as_ref().expect("Should have pre_rendered_chunk");

      for module_idx in &chunk.modules {
        let Some(module) = self.link_output.module_table[*module_idx].as_normal() else {
          continue;
        };

        if module.ecma_view.importers.is_empty() || module.ecma_view.dynamic_importers.is_empty() {
          continue;
        }

        let has_ineffective = module.ecma_view.dynamic_importers.iter().any(|importer_id| {
          !importer_id.as_path().is_in_node_modules()
            && pre_rendered_chunk.module_ids.contains(importer_id)
        });

        if has_ineffective {
          self.link_output.warnings.push(
            BuildDiagnostic::ineffective_dynamic_import(
              module.id.to_string(),
              module.ecma_view.importers.iter().map(ToString::to_string).collect(),
              module.ecma_view.dynamic_importers.iter().map(ToString::to_string).collect(),
            )
            .with_severity_warning(),
          );
        }
      }
    }
  }
}
