use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub struct LazyBarrelLargeReexports {
  pub module_id: String,
  pub reexport_count: usize,
}

impl BuildEvent for LazyBarrelLargeReexports {
  fn kind(&self) -> EventKind {
    EventKind::LazyBarrelLargeReexports
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    format!(
      "{} has {} re-exports. Eagerly resolving every entry can significantly slow down the build. Consider using `@rolldown/plugin-transform-imports` to rewrite imports at the source level so the barrel file is never loaded.",
      opts.stabilize_path(&self.module_id),
      self.reexport_count,
    )
  }

  fn on_diagnostic(
    &self,
    diagnostic: &mut crate::build_diagnostic::diagnostic::Diagnostic,
    _opts: &DiagnosticOptions,
  ) {
    diagnostic.helps.push(
      "See https://github.com/rolldown/plugins/tree/main/packages/transform-imports for usage."
        .to_string(),
    );
  }

  fn id(&self) -> Option<String> {
    Some(self.module_id.clone())
  }
}
