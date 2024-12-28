// cspell:ignore Estarget
use oxc::transformer::ESTarget as OxcEstarget;
use std::str::FromStr;

#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "deserialize_bundler_options", derive(Deserialize, JsonSchema))]
#[cfg_attr(feature = "deserialize_bundler_options", serde(rename_all = "camelCase"))]
pub enum ESTarget {
  Es5,
  Es2015,
  Es2016,
  Es2017,
  Es2018,
  Es2019,
  Es2020,
  Es2021,
  Es2022,
  Es2023,
  Es2024,
  #[default]
  EsNext,
}

impl FromStr for ESTarget {
  type Err = anyhow::Error;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "es5" => Ok(Self::Es5),
      "es2015" | "es6" => Ok(Self::Es2015),
      "es2016" => Ok(Self::Es2016),
      "es2017" => Ok(Self::Es2017),
      "es2018" => Ok(Self::Es2018),
      "es2019" => Ok(Self::Es2019),
      "es2020" => Ok(Self::Es2020),
      "es2021" => Ok(Self::Es2021),
      "es2022" => Ok(Self::Es2022),
      "es2023" => Ok(Self::Es2023),
      "es2024" => Ok(Self::Es2024),
      "esnext" => Ok(Self::EsNext),
      _ => Err(anyhow::anyhow!("Invalid target \"{s}\".")),
    }
  }
}

impl From<ESTarget> for OxcEstarget {
  fn from(value: ESTarget) -> OxcEstarget {
    match value {
      ESTarget::Es5 => OxcEstarget::ES5,
      ESTarget::Es2015 => OxcEstarget::ES2015,
      ESTarget::Es2016 => OxcEstarget::ES2016,
      ESTarget::Es2017 => OxcEstarget::ES2017,
      ESTarget::Es2018 => OxcEstarget::ES2018,
      ESTarget::Es2019 => OxcEstarget::ES2019,
      ESTarget::Es2020 => OxcEstarget::ES2020,
      ESTarget::Es2021 => OxcEstarget::ES2021,
      ESTarget::Es2022 => OxcEstarget::ES2022,
      ESTarget::Es2023 => OxcEstarget::ES2023,
      ESTarget::Es2024 => OxcEstarget::ES2024,
      ESTarget::EsNext => OxcEstarget::ESNext,
    }
  }
}
