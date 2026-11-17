use napi::bindgen_prelude::{FromNapiValue, TypeName, ValidateNapiValue};

#[derive(Debug)]
pub struct LimitedBooleanValue<const VALUE: bool>();

impl<const VALUE: bool> ValidateNapiValue for LimitedBooleanValue<VALUE> {}

impl<const VALUE: bool> TypeName for LimitedBooleanValue<VALUE> {
  fn type_name() -> &'static str {
    if VALUE { "True" } else { "False" }
  }

  fn value_type() -> napi::ValueType {
    napi::ValueType::Boolean
  }
}

impl<const VALUE: bool> FromNapiValue for LimitedBooleanValue<VALUE> {
  unsafe fn from_napi_value(
    env: napi::sys::napi_env,
    napi_val: napi::sys::napi_value,
  ) -> napi::Result<Self> {
    unsafe {
      let result = bool::from_napi_value(env, napi_val)?;
      if result == VALUE {
        Ok(Self())
      } else {
        Err(napi::Error::new(napi::Status::InvalidArg, "Invalid value".to_owned()))
      }
    }
  }
}

pub type BindingTrueValue = LimitedBooleanValue<true>;
