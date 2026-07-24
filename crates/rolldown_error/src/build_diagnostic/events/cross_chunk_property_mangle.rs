use std::fmt::Write;

use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

pub type CrossChunkPropertyMangleConflict = (String, Vec<(String, Option<String>)>);

#[derive(Debug)]
pub struct CrossChunkPropertyMangle {
  pub conflicts: Vec<CrossChunkPropertyMangleConflict>,
}

impl BuildEvent for CrossChunkPropertyMangle {
  fn kind(&self) -> EventKind {
    EventKind::CrossChunkPropertyMangle
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    const DISPLAY_LIMIT: usize = 5;

    let mut details = self
      .conflicts
      .iter()
      .take(DISPLAY_LIMIT)
      .map(|(original, mappings)| {
        let mappings = mappings
          .iter()
          .map(|(chunk, target)| match target {
            Some(target) => format!("{chunk:?} -> {target:?}"),
            None => format!("{chunk:?} -> unchanged"),
          })
          .collect::<Vec<_>>()
          .join(", ");
        format!("{original:?} ({mappings})")
      })
      .collect::<Vec<_>>()
      .join("; ");
    if self.conflicts.len() > DISPLAY_LIMIT {
      let _ = write!(details, "; and {} more", self.conflicts.len() - DISPLAY_LIMIT);
    }

    format!(
      "Property mangling assigned inconsistent names across chunks: {details}. Objects that cross those chunk boundaries can read or write the wrong property at runtime. Pin shared properties with `minify.mangleProps.cache`."
    )
  }
}
