#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug, Default)]
#[cfg_attr(feature = "deserialize_bundler_options", derive(Deserialize, JsonSchema))]
pub enum EsModuleType {
  True,
  False,
  #[default]
  IfDefaultProp,
}

impl From<String> for EsModuleType {
  fn from(value: String) -> Self {
    match value.as_str() {
      "true" => EsModuleType::True,
      "false" => EsModuleType::False,
      "if-default-prop" => EsModuleType::IfDefaultProp,
      _ => unreachable!("unknown es module type"),
    }
  }
}

impl From<bool> for EsModuleType {
  fn from(value: bool) -> Self {
    if value {
      EsModuleType::True
    } else {
      EsModuleType::False
    }
  }
}
