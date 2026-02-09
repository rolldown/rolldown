use std::fmt::Display;

use rolldown_ecmascript::PrintCommentsOptions;

#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

/// Resolved comments options with explicit boolean flags.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(from = "RawCommentsOption")
)]
pub struct CommentsOptions {
  /// Preserve legal comments (`@license`, `@preserve`, `//!`, `/*!`)
  pub legal: bool,
  /// Preserve annotation comments (`@__PURE__`, `@__NO_SIDE_EFFECTS__`, `@vite-ignore`, coverage directives)
  pub annotation: bool,
  /// Preserve JSDoc comments (`/** */`)
  pub other: bool,
}

impl Default for CommentsOptions {
  fn default() -> Self {
    // comments: true
    Self { legal: true, annotation: true, other: true }
  }
}

impl From<CommentsOptions> for PrintCommentsOptions {
  fn from(opts: CommentsOptions) -> Self {
    Self { legal: opts.legal, annotation: opts.annotation, other: opts.other }
  }
}

impl Display for CommentsOptions {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if self.legal && self.annotation && self.other {
      write!(f, "true")
    } else if !self.legal && !self.annotation && !self.other {
      write!(f, "false")
    } else {
      write!(
        f,
        "{{ legal: {}, annotation: {}, other: {} }}",
        self.legal, self.annotation, self.other
      )
    }
  }
}

/// Raw comments option as specified in config (bool or object).
/// Used for deserialization from JSON config files.
#[cfg(feature = "deserialize_bundler_options")]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum RawCommentsOption {
  Bool(bool),
  Object(RawCommentsObject),
}

#[cfg(feature = "deserialize_bundler_options")]
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RawCommentsObject {
  pub legal: Option<bool>,
  pub annotation: Option<bool>,
  pub other: Option<bool>,
}

#[cfg(feature = "deserialize_bundler_options")]
impl From<RawCommentsOption> for CommentsOptions {
  fn from(raw: RawCommentsOption) -> Self {
    match raw {
      RawCommentsOption::Bool(b) => CommentsOptions { legal: b, annotation: b, other: b },
      RawCommentsOption::Object(obj) => CommentsOptions {
        legal: obj.legal.unwrap_or(true),
        annotation: obj.annotation.unwrap_or(true),
        other: obj.other.unwrap_or(true),
      },
    }
  }
}
