use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub struct PreferBuiltinFeature {
  /// If it is none, it means there is no options to turn the feature off.
  pub builtin_feature: Option<String>,
  pub plugin_name: String,
  pub additional_message: Option<&'static str>,
}

impl BuildEvent for PreferBuiltinFeature {
  fn kind(&self) -> EventKind {
    EventKind::PreferBuiltinFeature
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    if let Some(feature) = &self.builtin_feature {
      format!(
        "Rolldown supports `{feature}` natively. Please refer https://rolldown.rs/reference/ for more details. It is more performant than passing `{}` to plugins option.{}",
        self.plugin_name,
        self.additional_message.unwrap_or_default()
      )
    } else {
      format!(
        "The functionality provided by `{}` is already covered natively, maybe you could remove the plugin from your configuration.{}",
        self.plugin_name,
        self.additional_message.unwrap_or_default()
      )
    }
  }

  fn on_diagnostic(
    &self,
    diagnostic: &mut crate::build_diagnostic::diagnostic::Diagnostic,
    _opts: &DiagnosticOptions,
  ) {
    diagnostic.helps.push(
      "This diagnostic may be false positive, you could turn it off via `checks.preferBuiltinFeature`".to_string(),
    );
  }
}
