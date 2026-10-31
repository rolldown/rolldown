use arcstr::ArcStr;
use oxc::span::Span;

use super::BuildEvent;
use crate::{
  build_diagnostic::diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions,
  types::event_kind::EventKind,
};

#[derive(Debug, Clone, Copy)]
pub enum InvalidImportAttributeKind {
  DynamicImportOptions,
  DynamicImportAttribute,
}

#[derive(Debug)]
pub struct InvalidImportAttribute {
  pub module_id: String,
  pub source: ArcStr,
  pub span: Span,
  pub kind: InvalidImportAttributeKind,
}

impl BuildEvent for InvalidImportAttribute {
  fn kind(&self) -> EventKind {
    EventKind::InvalidImportAttribute
  }

  fn id(&self) -> Option<String> {
    Some(self.module_id.clone())
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    let module_id = opts.stabilize_path(&self.module_id);
    match self.kind {
      InvalidImportAttributeKind::DynamicImportOptions => format!(
        "Rolldown could not statically analyze the options of a dynamic import in \"{module_id}\". Dynamic import options need to be an object with a nested `with` object, so the import has no attributes."
      ),
      InvalidImportAttributeKind::DynamicImportAttribute => format!(
        "Rolldown could not statically analyze an import attribute of a dynamic import in \"{module_id}\". Import attributes need to have string keys and values, so the attribute is ignored."
      ),
    }
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let file_id = diagnostic.add_file(opts.stabilize_path(&self.module_id), self.source.clone());
    let label = match self.kind {
      InvalidImportAttributeKind::DynamicImportOptions => "not statically analyzable",
      InvalidImportAttributeKind::DynamicImportAttribute => "not a string key and value",
    };
    diagnostic.add_label(&file_id, self.span.start..self.span.end, label.to_string());
  }
}
