use schemars::JsonSchema;
use serde::Deserialize;

use crate::{TestMeta, config_variant::ConfigVariant};

#[derive(Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools, clippy::pub_underscore_fields)]
pub struct TestConfig {
  #[serde(default)]
  pub config: rolldown_common::BundlerOptions,
  #[serde(default)]
  // Each config variant will be extended into the main config and executed.
  pub config_variants: Vec<ConfigVariant>,
  #[serde(default, flatten)]
  pub meta: TestMeta,
}
