use napi::Either;
use napi_derive::napi;
use rolldown_common::side_effects::HookSideEffects;

#[derive(Debug)]
#[napi(transparent)]
pub struct BindingHookSideEffects(Either<bool, String>);

impl TryFrom<BindingHookSideEffects> for HookSideEffects {
  type Error = napi::Error;
  fn try_from(value: BindingHookSideEffects) -> Result<Self, Self::Error> {
    Ok(match value.0 {
      Either::A(true) => Self::True,
      Either::A(false) => Self::False,
      Either::B(s) => match s.as_str() {
        "no-treeshake" => Self::NoTreeshake,
        _ => {
          return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("Invalid string option: {s}"),
          ));
        }
      },
    })
  }
}

impl From<HookSideEffects> for BindingHookSideEffects {
  fn from(value: HookSideEffects) -> Self {
    Self(match value {
      HookSideEffects::True => Either::A(true),
      HookSideEffects::False => Either::A(false),
      HookSideEffects::NoTreeshake => Either::B("no-treeshake".to_string()),
    })
  }
}

impl PartialEq for BindingHookSideEffects {
  fn eq(&self, other: &Self) -> bool {
    match (&self.0, &other.0) {
      (Either::A(a), Either::A(b)) => a == b,
      (Either::B(a), Either::B(b)) => a == b,
      _ => false,
    }
  }
}
