use schemars::JsonSchema;
use schemars::json_schema;
use serde::{Deserialize, Deserializer};

use crate::utils::true_by_default;

/// Controls which extended test variants are generated.
/// Can be `false` to disable all, `true` to enable all (default), or an object
/// to control each individually.
#[derive(Debug)]
pub struct ExtendedTests {
  /// Run the test case with the opposite value of `minifyInternalExports` compared to what the default would be.
  /// If it's explicitly set in the config, this option has no effect.
  /// If the default resolves to `true` (e.g., format: 'es' or minify: true), tests with `false`.
  /// If the default resolves to `false` (e.g., format: 'cjs' without minify), tests with `true`.
  pub opposite_minify_internal_exports: bool,
  /// Run the test case with `preserveEntrySignatures: 'strict'` in addition to the default.
  /// If `preserveEntrySignatures` is explicitly set in the config, this option has no effect.
  pub preserve_entry_signatures_strict: bool,
  /// Run the test case with `preserveEntrySignatures: 'allow-extension'` in addition to the default.
  /// If `preserveEntrySignatures` is explicitly set in the config, this option has no effect.
  pub preserve_entry_signatures_allow_extension: bool,
}

impl Default for ExtendedTests {
  fn default() -> Self {
    Self {
      opposite_minify_internal_exports: true,
      preserve_entry_signatures_strict: true,
      preserve_entry_signatures_allow_extension: true,
    }
  }
}

/// Inner struct for deserializing the object form and generating JSON Schema.
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExtendedTestsConfig {
  /// Run the test case with the opposite value of `minifyInternalExports` compared to what the default would be.
  /// If it's explicitly set in the config, this option has no effect.
  /// If the default resolves to `true` (e.g., format: 'es' or minify: true), tests with `false`.
  /// If the default resolves to `false` (e.g., format: 'cjs' without minify), tests with `true`.
  #[serde(default = "true_by_default")]
  opposite_minify_internal_exports: bool,
  /// Run the test case with `preserveEntrySignatures: 'strict'` in addition to the default.
  /// If `preserveEntrySignatures` is explicitly set in the config, this option has no effect.
  #[serde(default = "true_by_default")]
  preserve_entry_signatures_strict: bool,
  /// Run the test case with `preserveEntrySignatures: 'allow-extension'` in addition to the default.
  /// If `preserveEntrySignatures` is explicitly set in the config, this option has no effect.
  #[serde(default = "true_by_default")]
  preserve_entry_signatures_allow_extension: bool,
}

impl<'de> Deserialize<'de> for ExtendedTests {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    match &value {
      serde_json::Value::Bool(false) => Ok(Self {
        opposite_minify_internal_exports: false,
        preserve_entry_signatures_strict: false,
        preserve_entry_signatures_allow_extension: false,
      }),
      serde_json::Value::Bool(true) => Ok(Self::default()),
      serde_json::Value::Object(_) => {
        let config: ExtendedTestsConfig =
          serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(Self {
          opposite_minify_internal_exports: config.opposite_minify_internal_exports,
          preserve_entry_signatures_strict: config.preserve_entry_signatures_strict,
          preserve_entry_signatures_allow_extension: config
            .preserve_entry_signatures_allow_extension,
        })
      }
      _ => Err(serde::de::Error::custom("extendedTests must be a boolean or an object")),
    }
  }
}

impl JsonSchema for ExtendedTests {
  fn schema_name() -> std::borrow::Cow<'static, str> {
    "ExtendedTests".into()
  }

  fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    let config_schema = generator.subschema_for::<ExtendedTestsConfig>();
    json_schema!({
      "description": "Controls which extended test variants are generated.\nCan be `false` to disable all, `true` to enable all (default), or an object to control each individually.",
      "anyOf": [
        {
          "type": "boolean",
          "description": "`false` disables all extended tests, `true` enables all (default)."
        },
        config_schema
      ]
    })
  }
}
