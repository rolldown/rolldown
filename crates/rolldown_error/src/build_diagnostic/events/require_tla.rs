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
  pub importer_stable_id: ArcStr,
  pub importer_source: ArcStr,
  pub importee_stable_id: ArcStr,
  pub import_span: Span,
}

#[derive(Debug)]
pub struct RequireTla {
  pub importer_stable_id: ArcStr,
  pub importer_source: ArcStr,
  pub require_span: Span,
  pub tla_source_stable_id: ArcStr,
  pub tla_source_text: ArcStr,
  pub tla_keyword_span: Span,
  pub import_chain: Vec<ImportChainNote>,
}

impl RequireTla {
  fn is_direct(&self) -> bool {
    self.import_chain.is_empty()
  }
}

impl BuildEvent for RequireTla {
  fn kind(&self) -> EventKind {
    EventKind::RequireTlaError
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    let tla_path = opts.stabilize_path(self.tla_source_stable_id.as_str());
    if self.is_direct() {
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
    // A module can appear in multiple labels (e.g. the importer of the
    // require() call is also the TLA source in a self-require), and
    // `add_file` debug-asserts against duplicates — so register every
    // distinct source once up front, then only look up ids when adding
    // labels.
    let mut file_ids: FxHashMap<ArcStr, DiagnosticFileId> = FxHashMap::default();
    let files = std::iter::once((&self.importer_stable_id, &self.importer_source))
      .chain(self.import_chain.iter().map(|step| (&step.importer_stable_id, &step.importer_source)))
      .chain(std::iter::once((&self.tla_source_stable_id, &self.tla_source_text)));
    for (stable_id, source) in files {
      file_ids
        .entry(stable_id.clone())
        .or_insert_with(|| diagnostic.add_file(opts.stabilize_path(stable_id.as_str()), source));
    }

    diagnostic.add_label(
      &file_ids[&self.importer_stable_id],
      self.require_span.start..self.require_span.end,
      "The require() call is here:".to_string(),
    );

    for step in &self.import_chain {
      diagnostic.add_label(
        &file_ids[&step.importer_stable_id],
        step.import_span.start..step.import_span.end,
        format!(
          "The file \"{}\" imports the file \"{}\" here:",
          opts.stabilize_path(step.importer_stable_id.as_str()),
          opts.stabilize_path(step.importee_stable_id.as_str()),
        ),
      );
    }

    diagnostic.add_label(
      &file_ids[&self.tla_source_stable_id],
      self.tla_keyword_span.start..self.tla_keyword_span.end,
      format!(
        "The top-level await in \"{}\" is here:",
        opts.stabilize_path(self.tla_source_stable_id.as_str()),
      ),
    );
  }
}
