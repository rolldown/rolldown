use arcstr::ArcStr;
use oxc::span::Span;
use rustc_hash::FxHashMap;

use super::BuildEvent;
use crate::{
  build_diagnostic::diagnostic::{Diagnostic, DiagnosticFileId},
  types::diagnostic_options::DiagnosticOptions,
  types::event_kind::EventKind,
};

#[derive(Debug)]
pub struct ImportChainNote {
  pub importer_stable_id: String,
  pub importer_source: ArcStr,
  pub importee_stable_id: String,
  pub import_span: Span,
}

#[derive(Debug)]
pub struct RequireTla {
  pub importer_stable_id: String,
  pub importer_source: ArcStr,
  pub require_span: Span,
  pub tla_source_stable_id: String,
  pub tla_source_text: ArcStr,
  pub tla_keyword_span: Span,
  pub is_direct: bool,
  pub import_chain: Vec<ImportChainNote>,
}

impl BuildEvent for RequireTla {
  fn kind(&self) -> EventKind {
    EventKind::RequireTlaError
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    let tla_path = opts.stabilize_path(&self.tla_source_stable_id);
    if self.is_direct {
      format!(
        "This require call is not allowed because the imported file \"{tla_path}\" contains a top-level await",
      )
    } else {
      format!(
        "This require call is not allowed because the transitive dependency \"{tla_path}\" contains a top-level await",
      )
    }
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let mut file_ids: FxHashMap<String, DiagnosticFileId> = FxHashMap::default();

    let mut get_or_add_file =
      |diagnostic: &mut Diagnostic, stable_id: &str, source: &ArcStr| -> DiagnosticFileId {
        let path = opts.stabilize_path(stable_id);
        file_ids.entry(path.clone()).or_insert_with(|| diagnostic.add_file(path, source)).clone()
      };

    // Label 1: the require() call site
    let importer_file_id =
      get_or_add_file(diagnostic, &self.importer_stable_id, &self.importer_source);
    diagnostic.add_label(
      &importer_file_id,
      self.require_span.start..self.require_span.end,
      String::new(),
    );

    // Labels 2..N: the import chain (for transitive TLA)
    for step in &self.import_chain {
      let file_id = get_or_add_file(diagnostic, &step.importer_stable_id, &step.importer_source);
      diagnostic.add_label(
        &file_id,
        step.import_span.start..step.import_span.end,
        format!(
          "The file \"{}\" imports the file \"{}\" here:",
          opts.stabilize_path(&step.importer_stable_id),
          opts.stabilize_path(&step.importee_stable_id),
        ),
      );
    }

    // Final label: the top-level await keyword location
    let tla_file_id =
      get_or_add_file(diagnostic, &self.tla_source_stable_id, &self.tla_source_text);
    diagnostic.add_label(
      &tla_file_id,
      self.tla_keyword_span.start..self.tla_keyword_span.end,
      format!(
        "The top-level await in \"{}\" is here:",
        opts.stabilize_path(&self.tla_source_stable_id),
      ),
    );
  }
}
