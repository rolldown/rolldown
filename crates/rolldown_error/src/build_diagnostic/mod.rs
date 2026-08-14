pub mod constructors;
pub mod diagnostic;
pub mod events;

use rustc_hash::FxHashSet;
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

/// A mixed-severity diagnostic accumulator.
///
/// Unlike [`BatchedBuildDiagnostic`] — which is error-only and lives in the
/// `Result` `Err` channel — `Diagnostics` holds warnings, infos, and errors
/// together. Severity is read from each element via [`BuildDiagnostic::severity`],
/// so callers no longer need to thread a separate `errors` and `warnings` `Vec`
/// side by side.
///
/// At a drain checkpoint, [`Diagnostics::partition`] (or [`Diagnostics::into_result`])
/// splits the error-severity subset back out to feed the `Result` channel.
#[derive(Debug, Default)]
pub struct Diagnostics {
  diagnostics: Vec<BuildDiagnostic>,
  /// Cached: `true` once any error-severity diagnostic has been stored. Kept in
  /// sync by every mutator, so [`Diagnostics::has_errors`] is O(1) and
  /// [`Diagnostics::into_result`] can skip the partition scan on the common
  /// (no-error) path. A diagnostic's severity is frozen once stored — it is set
  /// only by the consuming `with_severity*` builders before `push`, and this
  /// type hands out no `&mut` to its elements — so the flag never goes stale.
  has_error: bool,
}

impl Diagnostics {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn push(&mut self, diagnostic: BuildDiagnostic) {
    self.has_error |= diagnostic.severity() == Severity::Error;
    self.diagnostics.push(diagnostic);
  }

  pub fn extend(&mut self, diagnostics: impl IntoIterator<Item = BuildDiagnostic>) {
    // Route through `push` so `has_error` stays in sync for each element.
    for diagnostic in diagnostics {
      self.push(diagnostic);
    }
  }

  pub fn is_empty(&self) -> bool {
    self.diagnostics.is_empty()
  }

  pub fn has_errors(&self) -> bool {
    self.has_error
  }

  /// Splits into `(warnings + infos, errors)`, preserving the relative order
  /// within each group. Because error- and warning-severity diagnostics were
  /// never interleaved into a single ordered stream before this type existed,
  /// merging then re-splitting yields the same two `Vec`s callers used to hold.
  pub fn partition(self) -> (Vec<BuildDiagnostic>, Vec<BuildDiagnostic>) {
    self.diagnostics.into_iter().partition(|d| d.severity() != Severity::Error)
  }

  /// Extracts all error-severity diagnostics, leaving the rest in `self`.
  pub fn extract_errors(&mut self) -> Vec<BuildDiagnostic> {
    self.has_error = false;
    self.diagnostics.extract_if(0.., |d| d.severity() == Severity::Error).collect()
  }

  /// Drain checkpoint: returns `Err(errors)` if any error-severity diagnostic is
  /// present, otherwise `Ok(warnings + infos)`. Mirrors the existing
  /// `if !errors.is_empty() { return Err(errors.into()) }` guard.
  ///
  /// When no error was ever stored, returns every diagnostic as-is without
  /// partitioning — the cached `has_error` flag makes this the common fast path.
  pub fn into_result(self) -> crate::BuildResult<Vec<BuildDiagnostic>> {
    if !self.has_error {
      return Ok(self.diagnostics);
    }
    let (warnings, errors) = self.partition();
    if errors.is_empty() { Ok(warnings) } else { Err(errors.into()) }
  }
}

impl From<Vec<BuildDiagnostic>> for Diagnostics {
  fn from(diagnostics: Vec<BuildDiagnostic>) -> Self {
    let has_error = diagnostics.iter().any(|d| d.severity() == Severity::Error);
    Self { diagnostics, has_error }
  }
}

impl IntoIterator for Diagnostics {
  type Item = BuildDiagnostic;
  type IntoIter = std::vec::IntoIter<BuildDiagnostic>;

  fn into_iter(self) -> Self::IntoIter {
    self.diagnostics.into_iter()
  }
}

/// Consolidates diagnostics by merging those that can be grouped together.
///
/// Currently consolidates:
/// - `TsConfigError` diagnostics with the same reason into a single diagnostic
pub fn consolidate_diagnostics(mut diagnostics: Vec<BuildDiagnostic>) -> Vec<BuildDiagnostic> {
  let mut seen_tsconfig_reasons = FxHashSet::<String>::default();
  diagnostics.retain_mut(|diag| {
    diag
      .downcast_mut::<TsConfigError>()
      .is_none_or(|tsconfig_err| seen_tsconfig_reasons.insert(tsconfig_err.reason.to_string()))
  });
  diagnostics
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
}
