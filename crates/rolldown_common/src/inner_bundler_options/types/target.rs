use oxc::transformer::ESTarget as OxcEstarget;

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

impl From<Vec<String>> for ESTarget {
  fn from(value: Vec<String>) -> ESTarget {
    for target in value {
      if target.len() <= 2 || !target[..2].eq_ignore_ascii_case("es") {
        continue;
      }

      let reset = &target[2..];

      if reset.eq_ignore_ascii_case("next") {
        return Self::EsNext;
      }

      if let Ok(n) = reset.parse::<usize>() {
        return match n {
          5 => ESTarget::Es5,
          6 | 2015 => ESTarget::Es2015,
          2016 => ESTarget::Es2016,
          2017 => ESTarget::Es2017,
          2018 => ESTarget::Es2018,
          2019 => ESTarget::Es2019,
          2020 => ESTarget::Es2020,
          2021 => ESTarget::Es2021,
          2022 => ESTarget::Es2022,
          2023 => ESTarget::Es2023,
          2024 => ESTarget::Es2024,
          _ => continue,
        };
      }
    }
    Self::EsNext
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
