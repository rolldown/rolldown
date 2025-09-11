use crate::{BuildDiagnostic, EventKindSwitcher};

pub fn filter_out_disabled_diagnostics(
  diagnostics: Vec<BuildDiagnostic>,
  switcher: &EventKindSwitcher,
) -> impl Iterator<Item = BuildDiagnostic> {
  diagnostics
    .into_iter()
    .filter(|d| switcher.contains(EventKindSwitcher::from_bits_truncate(1 << d.kind() as u32)))
}
