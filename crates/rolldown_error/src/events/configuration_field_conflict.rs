use crate::types::diagnostic_options::DiagnosticOptions;

use super::BuildEvent;

#[derive(Debug)]
/// Note that a has higher priority than b.
pub struct ConfigurationFieldConflict {
  pub a_field: String,
  pub a_config_name: String,
  pub b_field: String,
  pub b_config_name: String,
}

impl BuildEvent for ConfigurationFieldConflict {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::ConfigurationFieldConflict
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    let b_config_name = opts.stabilize_path(&self.b_config_name);
    let a_config_name = opts.stabilize_path(&self.a_config_name);
    format!(
      "{} in `{b_config_name}` will be override by {} in `{}` since `{}` has higher priority.\nMake sure this is what you expected",
      self.b_field, self.a_field, a_config_name, self.a_config_name
    )
  }
}
