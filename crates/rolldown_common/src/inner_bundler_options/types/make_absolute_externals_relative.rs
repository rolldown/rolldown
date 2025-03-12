#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "deserialize_bundler_options", derive(Deserialize, JsonSchema))]
pub enum MakeAbsoluteExternalsRelative {
  Bool(bool),
  #[default]
  IfRelativeSource,
}

impl MakeAbsoluteExternalsRelative {
  pub fn is_enabled(&self) -> bool {
    match self {
      MakeAbsoluteExternalsRelative::Bool(b) => *b,
      MakeAbsoluteExternalsRelative::IfRelativeSource => true,
    }
  }
}
