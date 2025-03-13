use crate::{BuildDiagnostic, EventKindSwitcher};

#[allow(unused)]
pub struct DiagnosticContainer {
  pub diagnostics: Vec<BuildDiagnostic>,
  pub event_kind_switcher: EventKindSwitcher,
}

#[allow(unused)]
impl DiagnosticContainer {
  pub fn new(switcher: EventKindSwitcher) -> Self {
    Self { diagnostics: vec![], event_kind_switcher: switcher }
  }

  pub fn push(&mut self, diagnostic: BuildDiagnostic) {
    if self
      .event_kind_switcher
      .contains(EventKindSwitcher::from_bits_truncate(1 << diagnostic.kind() as u32))
    {
      self.diagnostics.push(diagnostic);
    }
  }
}
