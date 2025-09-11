use std::fmt::Debug;

use arcstr::ArcStr;
use oxc::span::Span;

use crate::{
  build_diagnostic::diagnostic::Diagnostic,
  types::{diagnostic_options::DiagnosticOptions, event_kind::EventKind},
};

pub mod ambiguous_external_namespace;
pub mod assign_to_import;
pub mod bundler_initialize_error;
pub mod circular_dependency;
pub mod commonjs_variable_in_esm;
pub mod configuration_field_conflict;
pub mod empty_import_meta;
pub mod eval;
pub mod export_undefined_variable;
pub mod external_entry;
pub mod filename_conflict;
pub mod forbid_const_assign;
pub mod illegal_identifier_as_name;
pub mod import_is_undefined;
pub mod invalid_define_config;
pub mod invalid_export_option;
pub mod invalid_option;
pub mod json_parse;
pub mod missing_export;
pub mod missing_global_name;
pub mod missing_name_option_for_iife_export;
pub mod missing_name_option_for_umd_export;
pub mod mixed_export;
pub mod parse_error;
pub mod prefer_builtin_feature;
pub mod resolve_error;
pub mod unhandleable_error;
pub mod unloadable_dependency;
pub mod unresolved_entry;
pub mod unsupported_feature;

pub trait BuildEvent: Debug + Sync + Send {
  fn kind(&self) -> EventKind;

  fn message(&self, opts: &DiagnosticOptions) -> String;

  fn on_diagnostic(&self, _diagnostic: &mut Diagnostic, _opts: &DiagnosticOptions) {}

  // extra properties to match RollupLog interface
  // https://rollupjs.org/configuration-options/#onlog
  fn id(&self) -> Option<String> {
    None
  }

  fn exporter(&self) -> Option<String> {
    None
  }
}

impl<T: BuildEvent + 'static> From<T> for Box<dyn BuildEvent>
where
  Self: Sized,
{
  fn from(e: T) -> Self {
    Box::new(e)
  }
}

// --- TODO(hyf0): These errors are only for compatibility with legacy code. They should be replaced with more specific errors.
// NOTE: Using #[allow] instead of #[expect] because NapiError is only used with certain feature flags
#[allow(clippy::allow_attributes)] // Exemption: conditional usage based on features
#[allow(dead_code)]
#[derive(Debug)]
pub struct NapiError;

impl BuildEvent for NapiError {
  fn kind(&self) -> EventKind {
    EventKind::NapiError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    "Napi error".into()
  }
}

/// A Hybrid string type used for diagnostic, e.g.
/// for `UnresolvedError`, a specifier could be either a slice from raw source, or
/// created during ast transformation. When the specifier came from raw source, we could
/// use the `Span` information to give user better DX, otherwise, we could just use the string to
/// create a fallback message.
/// ## Panic
/// they type is only used for store information, user should check the span could be referenced
/// the raw source, or the user side may panic.
#[derive(Debug)]
pub enum DiagnosableArcstr {
  String(ArcStr),
  Span(Span),
}

// --- end
