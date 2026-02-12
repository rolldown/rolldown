use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub struct IneffectiveDynamicImport {
  pub module_id: String,
  pub static_importers: Vec<String>,
  pub dynamic_importers: Vec<String>,
}

impl IneffectiveDynamicImport {
  fn stable_list(ids: &[String], opts: &DiagnosticOptions, limit: usize) -> String {
    let stable: Vec<String> = ids.iter().map(|p| opts.stabilize_path(p)).collect();
    if stable.len() <= limit {
      stable.join(", ")
    } else {
      let mut result = stable[..limit].join(", ");
      result.push_str(", ...");
      result
    }
  }
}

impl BuildEvent for IneffectiveDynamicImport {
  fn kind(&self) -> EventKind {
    EventKind::IneffectiveDynamicImport
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    format!(
      "{} is dynamically imported by {} but also statically imported by {}, dynamic import will not move module into another chunk.",
      opts.stabilize_path(&self.module_id),
      Self::stable_list(&self.dynamic_importers, opts, 5),
      Self::stable_list(&self.static_importers, opts, 5),
    )
  }

  fn id(&self) -> Option<String> {
    Some(self.module_id.clone())
  }

  fn ids(&self) -> Option<Vec<String>> {
    Some([self.dynamic_importers.as_slice(), self.static_importers.as_slice()].concat())
  }
}
