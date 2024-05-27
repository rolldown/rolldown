use super::BuildEvent;

#[derive(Debug)]
pub struct UnresolvedOtherUnknown {
  pub(crate) message: String,
}

impl BuildEvent for UnresolvedOtherUnknown {
  fn kind(&self) -> crate::EventKind {
    crate::EventKind::UnresolvedJsonIllegally
  }

  fn message(&self, _opts: &crate::DiagnosticOptions) -> String {
    format!("Could not resolve message is -> \n {}", self.message)
  }
}
