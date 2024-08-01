use arcstr::ArcStr;
use oxc::span::Span;

use crate::{diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions};

use super::BuildEvent;

/// Only record start offset for efficiency and simplicity
#[derive(Debug)]
pub enum CjsExportStartOffset {
  Module(u32),
  Exports(u32),
}
impl CjsExportStartOffset {
  pub fn start(&self) -> u32 {
    match self {
      CjsExportStartOffset::Module(start) => *start,
      CjsExportStartOffset::Exports(start) => *start,
    }
  }

  pub fn end(&self) -> u32 {
    match self {
      CjsExportStartOffset::Module(start) => *start + 6,
      CjsExportStartOffset::Exports(start) => *start + 7,
    }
  }
}

#[derive(Debug)]
pub struct CommonJsVariableInEsm {
  pub filename: String,
  pub source: ArcStr,
  pub esm_export_span_start: u32,
  pub cjs_export_ident_start: CjsExportStartOffset,
}

impl BuildEvent for CommonJsVariableInEsm {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::CommonJsVariableInEsm
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    let variable = match self.cjs_export_ident_start {
      CjsExportStartOffset::Module(_) => "module",
      CjsExportStartOffset::Exports(_) => "exports",
    };
    format!("The CommonJS `{}` variable is treated as a global variable in an ECMAScript module and may not work as expected", variable)
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let filename = opts.stabilize_path(&self.filename);

    let file_id = diagnostic.add_file(filename, self.source.clone());
    diagnostic.add_label(
      &file_id,
      self.cjs_export_ident_start.start()..self.cjs_export_ident_start.end(),
      "".to_string(),
    );

    diagnostic.add_label(
      &file_id,
      self.esm_export_span_start..self.esm_export_span_start + /* length of `export` */6,
      "This file is considered to be an ECMAScript module because of the `export` keyword here:"
        .to_string(),
    );
  }
}
