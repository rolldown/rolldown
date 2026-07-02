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
  Info,
  Error,
  Warning,
}

pub struct BuildDiagnostic {
  inner: Box<dyn BuildEvent>,
  severity: Severity,
}

// `BuildEvent` is not `Debug` (dropping the supertrait lets the per-event `Debug`
// impls be dead-stripped from release builds), so format the diagnostic via its
// public accessors instead of the boxed event's `Debug`.
//
// We render through `to_diagnostic()` rather than reading `inner.message()`
// directly: for plugin-wrapped diagnostics `PluginError::message()` intentionally
// returns an empty string (the real text is injected later by `on_diagnostic`), so
// using the raw message here would print an empty `message`. `to_diagnostic()` runs
// `on_diagnostic`, which populates the real content. This is the error/cold path.
impl std::fmt::Debug for BuildDiagnostic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let diagnostic = self.to_diagnostic();
    // `inner` is rendered (via `to_diagnostic`) rather than printed directly, so the
    // struct is intentionally non-exhaustive over its raw fields.
    f.debug_struct("BuildDiagnostic")
      .field("severity", &self.severity)
      .field("kind", &diagnostic.kind)
      .field("message", &diagnostic.title)
      .finish_non_exhaustive()
  }
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

  pub fn plugin(&self) -> Option<String> {
    self.inner.plugin()
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
  pub fn with_severity(mut self, severity: Severity) -> Self {
    self.severity = severity;
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

#[derive(Default)]
pub struct BatchedBuildDiagnostic(Vec<BuildDiagnostic>);

impl std::fmt::Debug for BatchedBuildDiagnostic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple("BatchedBuildDiagnostic").field(&self.0).finish()
  }
}

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

#[cfg(test)]
mod tests {
  use super::BuildDiagnostic;
  use crate::build_diagnostic::events::plugin_error::CausedPlugin;

  // A plugin-wrapped diagnostic's inner `PluginError::message()` is intentionally
  // empty (the real text is injected via `on_diagnostic`). `Debug` must still render
  // the underlying error text, so render through `to_diagnostic()`.
  #[test]
  fn debug_renders_nested_plugin_diagnostic_message() {
    let inner =
      BuildDiagnostic::bundler_initialize_error("the underlying failure text".to_string(), None);
    let plugin_diag =
      BuildDiagnostic::plugin_error(CausedPlugin::new("my-plugin".into()), inner.into());

    let debug_output = format!("{plugin_diag:?}");
    assert!(
      debug_output.contains("the underlying failure text"),
      "expected non-empty underlying message in Debug output, got: {debug_output}"
    );
  }

  #[test]
  fn plugin_error_preserves_id_and_plugin() {
    let inner = BuildDiagnostic::missing_global_name(
      "/project/src/main.tsx".to_string(),
      "main".into(),
      "Main".into(),
    );
    assert_eq!(inner.id().as_deref(), Some("/project/src/main.tsx"));

    let plugin_diag = BuildDiagnostic::plugin_error(
      CausedPlugin::new("builtin:vite-transform".into()),
      inner.into(),
    );
    assert_eq!(plugin_diag.id().as_deref(), Some("/project/src/main.tsx"));
    assert_eq!(plugin_diag.plugin().as_deref(), Some("builtin:vite-transform"));
  }
}
