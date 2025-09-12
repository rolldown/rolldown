pub mod constructors;
pub mod diagnostic;
pub mod events;

use std::{
  fmt::Display,
  ops::{Deref, DerefMut},
};

use crate::{
  build_diagnostic::events::plugin_error::CausedPlugin,
  types::diagnostic_options::DiagnosticOptions, utils::downcast_napi_error_diagnostics,
};

use self::{diagnostic::Diagnostic, events::BuildEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
  Error,
  Warning,
}

#[derive(Debug)]
pub struct BuildDiagnostic {
  inner: Box<dyn BuildEvent>,
  severity: Severity,
}

impl std::error::Error for BuildDiagnostic {}

impl Display for BuildDiagnostic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.inner.message(&DiagnosticOptions::default()).fmt(f)
  }
}

impl BuildDiagnostic {
  fn new_inner(inner: impl Into<Box<dyn BuildEvent>>) -> Self {
    Self { inner: inner.into(), severity: Severity::Error }
  }

  pub fn id(&self) -> Option<String> {
    self.inner.id()
  }

  pub fn kind(&self) -> crate::types::event_kind::EventKind {
    self.inner.kind()
  }

  pub fn exporter(&self) -> Option<String> {
    self.inner.exporter()
  }

  pub fn severity(&self) -> Severity {
    self.severity
  }

  #[must_use]
  pub fn with_severity_warning(mut self) -> Self {
    self.severity = Severity::Warning;
    self
  }

  pub fn to_diagnostic(&self) -> Diagnostic {
    self.to_diagnostic_with(&DiagnosticOptions::default())
  }

  pub fn to_diagnostic_with(&self, opts: &DiagnosticOptions) -> Diagnostic {
    let mut diagnostic =
      Diagnostic::new(self.kind().to_string(), self.inner.message(opts), self.severity);
    self.inner.on_diagnostic(&mut diagnostic, opts);
    diagnostic
  }

  #[cfg(feature = "napi")]
  pub fn downcast_napi_error(&self) -> Result<&napi::Error, &Self> {
    self.inner.as_napi_error().ok_or(self)
  }
}

impl From<anyhow::Error> for BuildDiagnostic {
  fn from(err: anyhow::Error) -> Self {
    downcast_napi_error_diagnostics(err).unwrap_or_else(BuildDiagnostic::unhandleable_error)
  }
}

#[derive(Debug, Default)]
pub struct BatchedBuildDiagnostic(Vec<BuildDiagnostic>);

impl BatchedBuildDiagnostic {
  pub fn new(vec: Vec<BuildDiagnostic>) -> Self {
    Self(vec)
  }

  pub fn into_vec(self) -> Vec<BuildDiagnostic> {
    self.0
  }
}

impl std::error::Error for BatchedBuildDiagnostic {}

impl Display for BatchedBuildDiagnostic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.iter().map(std::string::ToString::to_string).collect::<Vec<_>>().join("\n").fmt(f)
  }
}

impl From<BuildDiagnostic> for BatchedBuildDiagnostic {
  fn from(v: BuildDiagnostic) -> Self {
    Self::new(vec![v])
  }
}

impl From<Vec<BuildDiagnostic>> for BatchedBuildDiagnostic {
  fn from(v: Vec<BuildDiagnostic>) -> Self {
    Self::new(v)
  }
}

impl From<anyhow::Error> for BatchedBuildDiagnostic {
  fn from(error: anyhow::Error) -> Self {
    let caused_plugin = error.downcast_ref::<CausedPlugin>().cloned();
    match error.downcast::<Self>() {
      Ok(batched) => {
        if let Some(plugin) = caused_plugin {
          Self::new(
            batched
              .into_vec()
              .into_iter()
              .map(|diag| BuildDiagnostic::plugin_error(plugin.clone(), diag.into()))
              .collect(),
          )
        } else {
          batched
        }
      }
      Err(error) => {
        // TODO: improve below logic
        let diagnostic = if let Some(plugin) = caused_plugin {
          downcast_napi_error_diagnostics(error)
            .unwrap_or_else(|error| BuildDiagnostic::plugin_error(plugin, error))
        } else {
          BuildDiagnostic::from(error)
        };
        Self::new(vec![diagnostic])
      }
    }
  }
}

impl Deref for BatchedBuildDiagnostic {
  type Target = Vec<BuildDiagnostic>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for BatchedBuildDiagnostic {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}
