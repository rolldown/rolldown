use arcstr::ArcStr;
use oxc::span::Span;

use crate::{diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions};

use super::BuildEvent;

/// Only record start offset for efficiency and simplicity
#[derive(Debug)]
pub enum CjsExportSpan {
  Module(Span),
  Exports(Span),
}
impl CjsExportSpan {
  pub fn start(&self) -> u32 {
    match self {
      CjsExportSpan::Module(span) | CjsExportSpan::Exports(span) => span.start,
    }
  }

  pub fn end(&self) -> u32 {
    match self {
      CjsExportSpan::Module(span) | CjsExportSpan::Exports(span) => span.end,
    }
  }
}

#[derive(Debug)]
pub struct CommonJsVariableInEsm {
  pub filename: String,
  pub source: ArcStr,
  pub esm_export_span: Span,
  pub cjs_export_ident_span: CjsExportSpan,
}

impl BuildEvent for CommonJsVariableInEsm {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::CommonJsVariableInEsm
  }

  fn id(&self) -> Option<String> {
    Some(self.filename.to_string())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    let variable = match self.cjs_export_ident_span {
      CjsExportSpan::Module(_) => "module",
      CjsExportSpan::Exports(_) => "exports",
    };
    format!(
      "The CommonJS `{variable}` variable is treated as a global variable in an ECMAScript module and may not work as expected"
    )
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let filename = opts.stabilize_path(&self.filename);

    let file_id = diagnostic.add_file(filename, self.source.clone());
    diagnostic.add_label(
      &file_id,
      self.cjs_export_ident_span.start()..self.cjs_export_ident_span.end(),
      String::new(),
    );

    diagnostic.add_label(
      &file_id,
      self.esm_export_span.start..self.esm_export_span.end,
      "This file is considered to be an ECMAScript module because of the `export` keyword here:"
        .to_string(),
    );
  }
}
