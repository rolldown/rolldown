use super::BuildEvent;

#[derive(Debug)]
pub struct UnsupportTarget {
  pub(crate) filename: String,
  pub(crate) fieldname: String,
  pub(crate) target: String,
}

impl BuildEvent for UnsupportTarget {
  fn kind(&self) -> crate::EventKind {
    crate::event_kind::EventKind::UnsupportedTarget
  }

  fn message(&self, _opts: &crate::DiagnosticOptions) -> String {
    format!("Unsupport {} {} for file {}", self.fieldname, self.target, self.filename)
  }
}
