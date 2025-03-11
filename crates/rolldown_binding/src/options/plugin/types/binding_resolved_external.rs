use rolldown_common::ResolvedExternal;

#[napi_derive::napi]
#[derive(Debug)]
pub enum BindingResolvedExternal {
  Bool(bool),
  Absolute,
  Relative,
}

impl From<BindingResolvedExternal> for ResolvedExternal {
  fn from(value: BindingResolvedExternal) -> Self {
    match value {
      BindingResolvedExternal::Bool(b) => ResolvedExternal::Bool(b),
      BindingResolvedExternal::Absolute => ResolvedExternal::Absolute,
      BindingResolvedExternal::Relative => ResolvedExternal::Relative,
    }
  }
}

impl From<ResolvedExternal> for BindingResolvedExternal {
  fn from(value: ResolvedExternal) -> Self {
    match value {
      ResolvedExternal::Bool(b) => BindingResolvedExternal::Bool(b),
      ResolvedExternal::Absolute => BindingResolvedExternal::Absolute,
      ResolvedExternal::Relative => BindingResolvedExternal::Relative,
    }
  }
}
