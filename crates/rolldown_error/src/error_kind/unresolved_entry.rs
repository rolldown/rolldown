use ariadne::Label;

use crate::{diagnostic::DiagnosticBuilder, PathExt};
use std::path::PathBuf;

use super::BuildErrorLike;

#[derive(Debug)]
pub struct UnresolvedEntry {
  pub(crate) unresolved_id: PathBuf,
}

impl BuildErrorLike for UnresolvedEntry {
  fn code(&self) -> &'static str {
    "UNRESOLVED_ENTRY"
  }

  fn message(&self) -> String {
    format!("Cannot resolve entry module {}.", self.unresolved_id.relative_display())
  }

  fn diagnostic_builder(&self) -> crate::diagnostic::DiagnosticBuilder {
    let module_specifier = self.unresolved_id.clone().display().to_string();
    let module_specifier_len = module_specifier.len();
    DiagnosticBuilder {
      code: Some(self.code()),
      summary: Some("Unresolved entry module".to_string()),
      files: Some(vec![("Output".to_string(), module_specifier)]),
      labels: Some(vec![Label::new(("Output".to_string(), 0..module_specifier_len))
        .with_message("Module Specifier")]),
      ..Default::default()
    }
  }
}
