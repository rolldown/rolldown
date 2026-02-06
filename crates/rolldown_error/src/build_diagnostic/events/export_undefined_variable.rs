use arcstr::ArcStr;
use oxc::span::Span;

use crate::{
  build_diagnostic::diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions,
};

use super::BuildEvent;

#[derive(Debug)]
pub struct ExportUndefinedVariable {
  pub filename: String,
  pub source: ArcStr,
  pub span: Span,
  pub name: ArcStr,
  pub similar_names: Vec<String>,
}

impl BuildEvent for ExportUndefinedVariable {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    crate::types::event_kind::EventKind::ExportUndefinedVariableError
  }

  fn id(&self) -> Option<String> {
    Some(self.filename.clone())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    let mut msg = format!("`{}` is not declared in this file", self.name);
    if !self.similar_names.is_empty() {
      msg.push_str(". Did you mean ");
      if self.similar_names.len() == 1 {
        msg.push_str(&format!("`{}`?", self.similar_names[0]));
      } else {
        msg.push_str("one of ");
        for (i, name) in self.similar_names.iter().enumerate() {
          if i > 0 {
            msg.push_str(", ");
          }
          msg.push_str(&format!("`{}`", name));
        }
        msg.push('?');
      }
    }
    msg
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let filename = opts.stabilize_path(&self.filename);

    let file_id = diagnostic.add_file(filename, self.source.clone());

    diagnostic.add_label(&file_id, self.span.start..self.span.end, String::new());
  }
}
