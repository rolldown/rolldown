/// Per-check configuration. Three canonical states:
///
/// - `Off` — the check is suppressed (matches `false` in user-facing config).
/// - `Warn` — the check emits a warning (matches `'warn'`).
/// - `Error` — the check fails the build (matches `'error'`).
///
/// Absent (`None` on the parent struct) means "use the check's built-in default".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckSetting {
  Off,
  Warn,
  Error,
}

#[cfg(feature = "deserialize_bundler_options")]
impl<'de> serde::Deserialize<'de> for CheckSetting {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Raw {
      Bool(bool),
      Str(String),
    }
    match Raw::deserialize(deserializer)? {
      Raw::Bool(false) => Ok(Self::Off),
      Raw::Bool(true) => Err(serde::de::Error::custom(
        "invalid check setting: `true` is not accepted, use 'warn' or 'error' instead",
      )),
      Raw::Str(s) => match s.as_str() {
        "warn" => Ok(Self::Warn),
        "error" => Ok(Self::Error),
        other => Err(serde::de::Error::custom(format!(
          "invalid check setting: expected `false`, 'warn', or 'error', got {other:?}"
        ))),
      },
    }
  }
}

#[cfg(feature = "deserialize_bundler_options")]
impl schemars::JsonSchema for CheckSetting {
  fn schema_name() -> std::borrow::Cow<'static, str> {
    "CheckSetting".into()
  }

  fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
      "description": "Per-check configuration. `false` disables the check, 'warn' emits a warning, 'error' fails the build.",
      "enum": [false, "warn", "error"]
    })
  }
}
