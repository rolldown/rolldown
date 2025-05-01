use rolldown_common::ModuleType;

pub enum OxcParseType {
  Js,
  Jsx,
  Ts,
  Tsx,
  Dts,
}

impl From<&ModuleType> for OxcParseType {
  fn from(module_type: &ModuleType) -> Self {
    match module_type {
      ModuleType::Jsx => OxcParseType::Jsx,
      ModuleType::Ts => OxcParseType::Ts,
      ModuleType::Tsx => OxcParseType::Tsx,
      ModuleType::Dts => OxcParseType::Dts,
      _ => OxcParseType::Js,
    }
  }
}
