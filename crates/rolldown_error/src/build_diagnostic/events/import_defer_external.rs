use arcstr::ArcStr;
use oxc::span::Span;

use super::BuildEvent;
use crate::{
  build_diagnostic::diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions,
  types::event_kind::EventKind,
};

/// `import defer` or `import.defer()` of an external module in ESM output, while
/// `experimental.importDeferExternal` is `'error'`. Rolldown cannot know if the target runtime
/// supports `import defer`, so the build fails instead of the bundle.
#[derive(Debug)]
pub struct ImportDeferExternal {
  pub(crate) importer_id: ArcStr,
  pub(crate) source: ArcStr,
  pub(crate) span: Span,
  pub(crate) external_id: ArcStr,
}

impl BuildEvent for ImportDeferExternal {
  fn kind(&self) -> EventKind {
    EventKind::ImportDeferExternalError
  }

  fn id(&self) -> Option<String> {
    Some(self.importer_id.to_string())
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    format!(
      "\"{}\" imports the external module \"{}\" in the defer phase. The ESM output keeps `import defer` for an external module only when `experimental.importDeferExternal` is `'preserve'`.",
      opts.stabilize_path(self.importer_id.as_str()),
      opts.stabilize_path(self.external_id.as_str()),
    )
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let file_id =
      diagnostic.add_file(opts.stabilize_path(self.importer_id.as_str()), self.source.clone());
    diagnostic.add_label(
      &file_id,
      self.span.start..self.span.end,
      "This deferred import has an external module.".to_string(),
    );
    diagnostic.add_help(
      "Set `experimental.importDeferExternal` to `'preserve'` when the target runtime supports `import defer`. Otherwise, bundle the module, or use the CommonJS output format, which loads the module lazily with `require()`."
        .to_string(),
    );
  }
}
