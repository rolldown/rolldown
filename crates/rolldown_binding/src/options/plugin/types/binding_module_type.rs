use napi::{bindgen_prelude::FromNapiValue, sys, JsString};
use rolldown::ModuleType;

#[derive(Debug, Clone)]
pub struct BindingModuleType(ModuleType);

impl AsRef<ModuleType> for BindingModuleType {
  fn as_ref(&self) -> &ModuleType {
    &self.0
  }
}

impl FromNapiValue for BindingModuleType {
  unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
    let value = JsString::from_napi_value(env, napi_val)?;
    Ok(Self(ModuleType::from_str_with_fallback(value.into_utf8()?.as_str()?)))
  }
}
