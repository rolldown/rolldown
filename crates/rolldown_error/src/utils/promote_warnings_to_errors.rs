use crate::{BuildDiagnostic, EventKindSwitcher, Severity};

/// Partitions `warnings` into `(remaining_warnings, promoted_errors)` based on whether
/// each diagnostic's kind is in `error_kinds`. Promoted diagnostics are mutated to
/// `Severity::Error` so downstream consumers render them as errors.
///
/// This is the single point of escalation for `checks.<x>: 'error'`: emission sites can
/// keep pushing warnings as-is, and the partition runs at the end of the build to move
/// the configured ones into the error channel.
pub fn promote_warnings_to_errors(
  warnings: Vec<BuildDiagnostic>,
  error_kinds: &EventKindSwitcher,
) -> (Vec<BuildDiagnostic>, Vec<BuildDiagnostic>) {
  let mut remaining = Vec::with_capacity(warnings.len());
  let mut promoted = Vec::new();
  for mut diag in warnings {
    let bit = EventKindSwitcher::from_bits_truncate(1 << diag.kind() as u32);
    if error_kinds.contains(bit) {
      diag.set_severity(Severity::Error);
      promoted.push(diag);
    } else {
      remaining.push(diag);
    }
  }
  (remaining, promoted)
}
