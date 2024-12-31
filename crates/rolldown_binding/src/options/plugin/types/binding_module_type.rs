use std::ops::{Deref, DerefMut};

use napi::{bindgen_prelude::FromNapiValue, sys, JsString};
use rolldown::ModuleType;

#[derive(Debug, Clone)]
pub struct BindingModuleType(ModuleType);

impl Deref for BindingModuleType {
  type Target = ModuleType;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for BindingModuleType {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl FromNapiValue for BindingModuleType {
  unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
    let value = JsString::from_napi_value(env, napi_val)?;
    Ok(Self(ModuleType::from_str_with_fallback(value.into_utf8()?.as_str()?)))
  }
}
