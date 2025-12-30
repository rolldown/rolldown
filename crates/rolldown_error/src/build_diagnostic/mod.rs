pub mod constructors;
pub mod diagnostic;
pub mod events;

use std::fmt::Display;

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

  pub fn ids(&self) -> Option<Vec<String>> {
    self.inner.ids()
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

  pub fn to_message_with(&self, opts: &DiagnosticOptions) -> String {
    self.inner.message(opts)
  }

  #[cfg(feature = "napi")]
  pub fn downcast_napi_error(&self) -> Result<&napi::Error, &Self> {
    self.inner.as_napi_error().ok_or(self)
  }

  /// Attempt to downcast the inner event to a specific type.
  pub fn downcast_mut<T: 'static + BuildEvent>(&mut self) -> Option<&mut T> {
    self.inner.as_any_mut().downcast_mut()
  }
}

impl From<anyhow::Error> for BuildDiagnostic {
  fn from(err: anyhow::Error) -> Self {
    downcast_napi_error_diagnostics(err).unwrap_or_else(BuildDiagnostic::unhandleable_error)
  }
}

#[derive(Debug)]
pub enum BuildError {
  Single(BuildDiagnostic),
  Multi(Vec<BuildDiagnostic>),
}

impl std::error::Error for BuildError {}

impl Display for BuildError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      BuildError::Single(diag) => diag.fmt(f),
      BuildError::Multi(diags) => {
        diags.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n").fmt(f)
      }
    }
  }
}

impl BuildError {
  pub fn into_vec(self) -> Vec<BuildDiagnostic> {
    match self {
      BuildError::Single(diag) => vec![diag],
      BuildError::Multi(diags) => diags,
    }
  }

  fn as_slice(&self) -> &[BuildDiagnostic] {
    match self {
      BuildError::Single(diag) => std::slice::from_ref(diag),
      BuildError::Multi(diags) => diags,
    }
  }

  #[inline]
  pub fn iter(&self) -> impl Iterator<Item = &BuildDiagnostic> {
    self.as_slice().iter()
  }

  #[inline]
  pub fn extend_into(self, target: &mut Vec<BuildDiagnostic>) {
    match self {
      Self::Single(diag) => target.push(diag),
      Self::Multi(vec) => target.extend(vec),
    }
  }

  pub fn is_error_severity_only(&self) -> bool {
    match self {
      BuildError::Single(diag) => diag.severity() == Severity::Error,
      BuildError::Multi(diags) => diags.iter().all(|diag| diag.severity() == Severity::Error),
    }
  }
}

impl From<BuildDiagnostic> for BuildError {
  fn from(err: BuildDiagnostic) -> Self {
    Self::Single(err)
  }
}

impl From<Vec<BuildDiagnostic>> for BuildError {
  fn from(errs: Vec<BuildDiagnostic>) -> Self {
    Self::Multi(errs)
  }
}

impl From<anyhow::Error> for BuildError {
  fn from(error: anyhow::Error) -> Self {
    let caused_plugin = error.downcast_ref::<CausedPlugin>().cloned();
    match error.downcast::<Self>() {
      Ok(batched) => {
        if let Some(plugin) = caused_plugin {
          Self::Multi(
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
        Self::Single(diagnostic)
      }
    }
  }
}

impl From<BuildError> for Vec<BuildDiagnostic> {
  fn from(err: BuildError) -> Self {
    match err {
      BuildError::Single(diag) => vec![diag],
      BuildError::Multi(diags) => diags,
    }
  }
}
