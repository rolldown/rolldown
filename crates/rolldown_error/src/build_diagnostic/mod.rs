pub mod constructors;
pub mod diagnostic;
pub mod events;

use std::{
  borrow::Cow,
  fmt::Display,
  ops::{Deref, DerefMut},
};

use crate::types::diagnostic_options::DiagnosticOptions;

use self::{diagnostic::Diagnostic, events::BuildEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
  Error,
  Warning,
}

#[derive(Debug, Clone)]
pub struct CausedPlugin {
  pub name: Cow<'static, str>,
}

impl CausedPlugin {
  pub fn new(name: Cow<'static, str>) -> Self {
    Self { name }
  }
}

#[derive(Debug)]
pub struct BuildDiagnostic {
  inner: Box<dyn BuildEvent>,
  severity: Severity,
  caused_plugin: Option<CausedPlugin>,
}

impl Display for BuildDiagnostic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.inner.message(&DiagnosticOptions::default()).fmt(f)
  }
}

impl BuildDiagnostic {
  fn new_inner(inner: impl Into<Box<dyn BuildEvent>>) -> Self {
    Self { inner: inner.into(), severity: Severity::Error, caused_plugin: None }
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
  pub fn with_caused_plugin(mut self, plugin: CausedPlugin) -> Self {
    self.caused_plugin = Some(plugin);
    self
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
    if let Some(plugin) = &self.caused_plugin {
      diagnostic.kind = plugin.name.to_string();
    }
    self.inner.on_diagnostic(&mut diagnostic, opts);
    diagnostic
  }

  #[cfg(feature = "napi")]
  pub fn downcast_napi_error(&self) -> Result<&napi::Error, &Self> {
    self.inner.as_napi_error().ok_or(self)
  }
}

// Direct implementation for `anyhow::Error` to allow using `?` operator.
// Note: `anyhow::Error` does NOT implement `std::error::Error`, so this doesn't conflict
// with any blanket implementations. For other error types, use `.map_err_to_unhandleable()`.
impl From<anyhow::Error> for BuildDiagnostic {
  fn from(error: anyhow::Error) -> Self {
    #[cfg(feature = "napi")]
    {
      match error.downcast::<napi::Error>() {
        Ok(err) => BuildDiagnostic::napi_error(err),
        Err(err) => BuildDiagnostic::unhandleable_error(err),
      }
    }

    #[cfg(not(feature = "napi"))]
    {
      BuildDiagnostic::unhandleable_error(error)
    }
  }
}

#[cfg(feature = "napi")]
impl From<napi::Error> for BuildDiagnostic {
  fn from(error: napi::Error) -> Self {
    BuildDiagnostic::napi_error(error)
  }
}

#[cfg(feature = "napi")]
impl From<BuildDiagnostic> for napi::Error {
  fn from(error: BuildDiagnostic) -> Self {
    match error.downcast_napi_error() {
      Ok(napi_error) => {
        #[cfg(not(target_family = "wasm"))]
        {
          napi_error.try_clone().unwrap_or_else(|e| e)
        }
        #[cfg(target_family = "wasm")]
        {
          napi::Error::new(napi_error.status, napi_error.reason.clone())
        }
      }
      Err(error) => napi::Error::from_reason(error.to_diagnostic().to_color_string()),
    }
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

impl Display for BatchedBuildDiagnostic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.iter().map(std::string::ToString::to_string).collect::<Vec<_>>().join("\n").fmt(f)
  }
}

impl From<anyhow::Error> for BatchedBuildDiagnostic {
  fn from(error: anyhow::Error) -> Self {
    #[cfg(feature = "napi")]
    {
      match error.downcast::<napi::Error>() {
        Ok(err) => BuildDiagnostic::napi_error(err).into(),
        Err(err) => BuildDiagnostic::unhandleable_error(err).into(),
      }
    }

    #[cfg(not(feature = "napi"))]
    {
      BuildDiagnostic::unhandleable_error(error).into()
    }
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
