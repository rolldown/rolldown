pub mod constructors;
pub mod diagnostic;
pub mod events;

use rustc_hash::FxHashMap;
use std::{
  fmt::Display,
  ops::{Deref, DerefMut},
};

use crate::{
  build_diagnostic::events::plugin_error::CausedPlugin,
  types::diagnostic_options::DiagnosticOptions, utils::downcast_napi_error_diagnostics,
};

use self::{diagnostic::Diagnostic, events::BuildEvent, events::tsconfig_error::TsConfigError};

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
  pub fn downcast_ref<T: 'static + BuildEvent>(&self) -> Option<&T> {
    self.inner.as_any().downcast_ref()
  }

  /// Attempt to downcast the inner event to a specific type (mutable).
  pub fn downcast_mut<T: 'static + BuildEvent>(&mut self) -> Option<&mut T> {
    self.inner.as_any_mut().downcast_mut()
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

/// Consolidates diagnostics by merging those that can be grouped together.
///
/// Currently consolidates:
/// - `TsConfigError` diagnostics with the same reason into a single diagnostic
///   listing all affected files
pub fn consolidate_diagnostics(diagnostics: Vec<BuildDiagnostic>) -> Vec<BuildDiagnostic> {
  let mut tsconfig_map = FxHashMap::<String, usize>::default();
  let mut result: Vec<BuildDiagnostic> = Vec::new();
  for mut diag in diagnostics {
    if let Some(tsconfig_err) = diag.downcast_mut::<TsConfigError>() {
      let reason_key = tsconfig_err.reason.to_string();

      if let Some(&idx) = tsconfig_map.get(&reason_key) {
        if let Some(existing_tsconfig) = result[idx].downcast_mut::<TsConfigError>() {
          existing_tsconfig.merge(std::mem::take(&mut tsconfig_err.file_paths));
        }
      } else {
        tsconfig_map.insert(reason_key, result.len());
        result.push(diag);
      }
    } else {
      result.push(diag);
    }
  }
  result
}
