use rolldown_common::ModuleType;

pub enum OxcParseType {
  Js,
  Jsx,
  Ts,
  Tsx,
}

impl From<&ModuleType> for OxcParseType {
  fn from(module_type: &ModuleType) -> Self {
    match module_type {
      ModuleType::Jsx => OxcParseType::Jsx,
      ModuleType::Ts => OxcParseType::Ts,
      ModuleType::Tsx => OxcParseType::Tsx,
      _ => OxcParseType::Js,
    }
  }
}
