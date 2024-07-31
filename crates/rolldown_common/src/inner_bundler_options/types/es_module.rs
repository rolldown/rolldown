#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug, Default)]
#[cfg_attr(feature = "deserialize_bundler_options", derive(Deserialize, JsonSchema))]
pub enum EsModuleType {
  Always,
  Never,
  #[default]
  IfDefaultProp,
}

impl From<String> for EsModuleType {
  fn from(value: String) -> Self {
    match value.as_str() {
      "always" => EsModuleType::Always,
      "never" => EsModuleType::Never,
      "if-default-prop" => EsModuleType::IfDefaultProp,
      _ => unreachable!("unknown es module type"),
    }
  }
}
