use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub struct PreferBuiltinFeature {
  /// If it is none, it means there is no options to turn the feature off.
  pub builtin_feature: Option<String>,
  pub plugin_name: String,
}

impl BuildEvent for PreferBuiltinFeature {
  fn kind(&self) -> EventKind {
    EventKind::PreferBuiltinFeature
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    if let Some(feature) = &self.builtin_feature {
      format!(
        "Rolldown supports `{feature}` natively, please refer https://rolldown.rs/reference/config-options for more details, this is performant than passing `{}` to plugins option.",
        self.plugin_name
      )
    } else {
      format!(
        "The functionality provided by `{}` is already covered natively, maybe you could remove the plugin from your configuration",
        self.plugin_name
      )
    }
  }

  fn on_diagnostic(
    &self,
    _diagnostic: &mut crate::build_diagnostic::diagnostic::Diagnostic,
    _opts: &DiagnosticOptions,
  ) {
    _diagnostic.help = Some(
      "This diagnostic may be false positive, you could turn it off via `checks.preferBuiltinFeature`".to_string(),
    );
  }
}
