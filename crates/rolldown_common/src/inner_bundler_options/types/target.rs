use oxc::transformer::ESTarget as OxcESTarget;

#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "deserialize_bundler_options", derive(Deserialize, JsonSchema))]
#[cfg_attr(feature = "deserialize_bundler_options", serde(rename_all = "lowercase"))]
pub enum ESTarget {
  ES5,
  ES2015,
  ES2016,
  ES2017,
  ES2018,
  ES2019,
  ES2020,
  ES2021,
  ES2022,
  ES2023,
  ES2024,
  #[default]
  ESNext,
}

impl TryFrom<ESTarget> for OxcESTarget {
  type Error = &'static str;
  fn try_from(value: ESTarget) -> Result<Self, Self::Error> {
    Ok(match value {
      ESTarget::ES5 => return Err("ES5 is not yet supported."),
      ESTarget::ES2015 => OxcESTarget::ES2015,
      ESTarget::ES2016 => OxcESTarget::ES2016,
      ESTarget::ES2017 => OxcESTarget::ES2017,
      ESTarget::ES2018 => OxcESTarget::ES2018,
      ESTarget::ES2019 => OxcESTarget::ES2019,
      ESTarget::ES2020 => OxcESTarget::ES2020,
      ESTarget::ES2021 => OxcESTarget::ES2021,
      ESTarget::ES2022 => OxcESTarget::ES2022,
      ESTarget::ES2023 => OxcESTarget::ES2023,
      ESTarget::ES2024 => OxcESTarget::ES2024,
      ESTarget::ESNext => OxcESTarget::ESNext,
    })
  }
}
