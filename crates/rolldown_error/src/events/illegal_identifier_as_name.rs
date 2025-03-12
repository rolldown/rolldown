use crate::events::BuildEvent;
use crate::{DiagnosticOptions, EventKind};
use arcstr::ArcStr;

#[derive(Debug)]
pub struct IllegalIdentifierAsName {
  pub(crate) identifier_name: ArcStr,
}

impl BuildEvent for IllegalIdentifierAsName {
  fn kind(&self) -> EventKind {
    EventKind::IllegalIdentifierAsNameError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      r#"Given name "{}" is not a legal JS identifier. If you need this, you can try "output.extend: true"."#,
      self.identifier_name
    )
  }
}
