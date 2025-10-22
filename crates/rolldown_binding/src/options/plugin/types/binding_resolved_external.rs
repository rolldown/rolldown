use napi::Either;
use rolldown_common::ResolvedExternal;
use rolldown_error::BuildDiagnostic;

#[derive(Debug)]
#[napi_derive::napi(transparent)]
pub struct BindingResolvedExternal(Either<bool, String>);

impl TryFrom<BindingResolvedExternal> for ResolvedExternal {
  type Error = BuildDiagnostic;

  fn try_from(value: BindingResolvedExternal) -> Result<Self, Self::Error> {
    Ok(match value.0 {
      Either::A(b) => Self::Bool(b),
      Either::B(s) => match s.as_str() {
        "absolute" => Self::Absolute,
        "relative" => Self::Relative,
        _ => {
          return Err(BuildDiagnostic::napi_error(napi::Error::new(
            napi::Status::InvalidArg,
            format!("Invalid string option: {s}"),
          )));
        }
      },
    })
  }
}

impl From<ResolvedExternal> for BindingResolvedExternal {
  fn from(value: ResolvedExternal) -> Self {
    Self(match value {
      ResolvedExternal::Bool(b) => Either::A(b),
      ResolvedExternal::Absolute => Either::B("absolute".to_string()),
      ResolvedExternal::Relative => Either::B("relative".to_string()),
    })
  }
}
