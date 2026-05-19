use std::{ptr, sync::Arc};

use arcstr::ArcStr;
use napi::{
  Error, Result, Status,
  bindgen_prelude::{FromNapiValue, ToNapiValue, TypeName, ValidateNapiValue, ValueType},
  sys,
};

#[derive(Debug, Clone)]
pub struct BindingSharedString {
  inner: SharedStringInner,
}

#[derive(Debug, Clone)]
enum SharedStringInner {
  ArcStr(ArcStr),
  #[expect(clippy::rc_buffer, reason = "Arc<String> lets renderChunk recover owned code")]
  String(Arc<String>),
}

impl BindingSharedString {
  fn as_str(&self) -> &str {
    match &self.inner {
      SharedStringInner::ArcStr(value) => value.as_str(),
      SharedStringInner::String(value) => value.as_str(),
    }
  }
}

impl From<ArcStr> for BindingSharedString {
  fn from(value: ArcStr) -> Self {
    Self { inner: SharedStringInner::ArcStr(value) }
  }
}

impl From<Arc<String>> for BindingSharedString {
  fn from(value: Arc<String>) -> Self {
    Self { inner: SharedStringInner::String(value) }
  }
}

unsafe fn create_napi_string(env: sys::napi_env, value: &str) -> Result<sys::napi_value> {
  let mut ptr = ptr::null_mut();
  let status = unsafe {
    sys::napi_create_string_utf8(
      env,
      value.as_ptr().cast(),
      value.len().cast_signed(),
      &raw mut ptr,
    )
  };

  if status != sys::Status::napi_ok {
    return Err(Error::new(
      Status::from(status),
      "Failed to convert shared rust string into napi `string`",
    ));
  }

  Ok(ptr)
}

impl TypeName for BindingSharedString {
  fn type_name() -> &'static str {
    "String"
  }

  fn value_type() -> ValueType {
    ValueType::String
  }
}

impl ValidateNapiValue for BindingSharedString {}

impl FromNapiValue for BindingSharedString {
  unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> Result<Self> {
    Ok(Self::from(Arc::new(unsafe { String::from_napi_value(env, napi_val)? })))
  }
}

impl ToNapiValue for &BindingSharedString {
  unsafe fn to_napi_value(env: sys::napi_env, value: Self) -> Result<sys::napi_value> {
    unsafe { create_napi_string(env, value.as_str()) }
  }
}

impl ToNapiValue for &mut BindingSharedString {
  unsafe fn to_napi_value(env: sys::napi_env, value: Self) -> Result<sys::napi_value> {
    unsafe { ToNapiValue::to_napi_value(env, &*value) }
  }
}

impl ToNapiValue for BindingSharedString {
  unsafe fn to_napi_value(env: sys::napi_env, value: Self) -> Result<sys::napi_value> {
    unsafe { ToNapiValue::to_napi_value(env, &value) }
  }
}
