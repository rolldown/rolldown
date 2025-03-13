use rolldown_common::MakeAbsoluteExternalsRelative;

#[napi_derive::napi]
#[derive(Debug)]
pub enum BindingMakeAbsoluteExternalsRelative {
  Bool(bool),
  IfRelativeSource,
}

impl From<BindingMakeAbsoluteExternalsRelative> for MakeAbsoluteExternalsRelative {
  fn from(value: BindingMakeAbsoluteExternalsRelative) -> Self {
    match value {
      BindingMakeAbsoluteExternalsRelative::Bool(b) => MakeAbsoluteExternalsRelative::Bool(b),
      BindingMakeAbsoluteExternalsRelative::IfRelativeSource => {
        MakeAbsoluteExternalsRelative::IfRelativeSource
      }
    }
  }
}
