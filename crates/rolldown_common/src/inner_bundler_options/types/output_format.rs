use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub enum OutputFormat {
  Esm,
  Cjs,
}
