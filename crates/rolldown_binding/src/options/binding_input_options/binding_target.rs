#[derive(Debug)]
#[napi_derive::napi(string_enum)]
pub enum BindingTarget {
  // ES5,
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
  ESNext,
}

impl From<BindingTarget> for rolldown::ESTarget {
  fn from(value: BindingTarget) -> Self {
    match value {
      BindingTarget::ES2015 => rolldown::ESTarget::Es2015,
      BindingTarget::ES2016 => rolldown::ESTarget::Es2016,
      BindingTarget::ES2017 => rolldown::ESTarget::Es2017,
      BindingTarget::ES2018 => rolldown::ESTarget::Es2018,
      BindingTarget::ES2019 => rolldown::ESTarget::Es2019,
      BindingTarget::ES2020 => rolldown::ESTarget::Es2020,
      BindingTarget::ES2021 => rolldown::ESTarget::Es2021,
      BindingTarget::ES2022 => rolldown::ESTarget::Es2022,
      BindingTarget::ES2023 => rolldown::ESTarget::Es2023,
      BindingTarget::ES2024 => rolldown::ESTarget::Es2024,
      BindingTarget::ESNext => rolldown::ESTarget::EsNext,
    }
  }
}
