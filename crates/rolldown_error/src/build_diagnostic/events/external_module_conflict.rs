use arcstr::ArcStr;
use oxc::span::Span;

use super::BuildEvent;
use crate::{
  build_diagnostic::diagnostic::Diagnostic,
  types::{diagnostic_options::DiagnosticOptions, event_kind::EventKind},
};

/// This struct holds the import that causes the conflict. The import resolves the
/// module id to the other kind (external or not external).
#[derive(Debug)]
pub struct ExternalModuleConflictImporter {
  pub source: ArcStr,
  pub importer_id: ArcStr,
  pub importee_span: Span,
}

/// Two resolutions of one module id do not agree. One resolution is external, and the other
/// resolution is not external.
///
/// A module id has one module, and the first resolution decides if the module is external.
/// A later resolution of the other kind is an error, like in Rollup. Without this error, the
/// load order decides the module kind.
///
/// This event does not implement `BuildEvent::id`, because Rollup logs for these codes do not
/// have an `id` field.
#[derive(Debug)]
pub struct ExternalModuleConflict {
  pub(crate) module_id: ArcStr,
  /// The value is `true` if the existing module is external and a later resolution is not
  /// external.
  pub(crate) is_existing_module_external: bool,
  pub(crate) importer: Option<ExternalModuleConflictImporter>,
}

impl BuildEvent for ExternalModuleConflict {
  fn kind(&self) -> EventKind {
    if self.is_existing_module_external {
      EventKind::ExternalModulesCannotBeTransformedToModulesError
    } else {
      EventKind::InvalidExternalIdError
    }
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    let module_id = opts.stabilize_path(self.module_id.as_str());
    match (self.is_existing_module_external, &self.importer) {
      (false, Some(importer)) => format!(
        "\"{module_id}\" is imported as an external by \"{}\", but is already an existing non-external module id.",
        importer.importer_id
      ),
      (false, None) => format!(
        "\"{module_id}\" is resolved as an external, but is already an existing non-external module id."
      ),
      (true, _) => {
        format!(
          "\"{module_id}\" is resolved as a module now, but it was an external module before."
        )
      }
    }
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, _opts: &DiagnosticOptions) {
    if let Some(importer) = &self.importer {
      let importer_file =
        diagnostic.add_file(importer.importer_id.clone(), importer.source.clone());
      let label = if self.is_existing_module_external {
        "This import resolves to a non-external module."
      } else {
        "This import resolves to an external module."
      };
      diagnostic.add_label(
        &importer_file,
        importer.importee_span.start..importer.importee_span.end,
        label.to_string(),
      );
    }
    diagnostic.add_help(
      "Make all imports of this module resolve the same way: all as external, or all as non-external."
        .to_string(),
    );
  }
}
