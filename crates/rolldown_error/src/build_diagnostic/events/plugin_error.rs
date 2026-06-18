use std::{borrow::Cow, fmt::Display};

use crate::{
  BuildDiagnostic,
  build_diagnostic::diagnostic,
  types::{diagnostic_options::DiagnosticOptions, event_kind::EventKind},
};

use super::BuildEvent;

#[derive(Debug, Clone)]
pub struct CausedPlugin {
  pub name: Cow<'static, str>,
}

impl CausedPlugin {
  pub fn new(name: Cow<'static, str>) -> Self {
    Self { name }
  }
}

impl Display for CausedPlugin {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "plugin `{}` threw an error", self.name)
  }
}

#[derive(Debug)]
pub struct PluginError {
  pub(crate) plugin: CausedPlugin,
  pub(crate) error: anyhow::Error,
}

impl BuildEvent for PluginError {
  fn kind(&self) -> EventKind {
    EventKind::PluginError
  }

  fn id(&self) -> Option<String> {
    // A `PluginError` wraps an inner diagnostic (e.g. an `OxcError` carrying the
    // offending file path). Delegate to it so the file id is preserved; otherwise
    // it defaults to `None` and is lost in the error object exposed to JS.
    self.error.downcast_ref::<BuildDiagnostic>().and_then(BuildDiagnostic::id)
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    if self.error.downcast_ref::<BuildDiagnostic>().is_some() {
      String::default()
    } else {
      format!("{:?}", self.error)
    }
  }

  fn on_diagnostic(&self, diagnostic: &mut diagnostic::Diagnostic, opts: &DiagnosticOptions) {
    if let Some(err) = self.error.downcast_ref::<BuildDiagnostic>() {
      *diagnostic = err.to_diagnostic_with(opts);
    }
    diagnostic.kind = self.plugin.name.to_string();
  }
}
