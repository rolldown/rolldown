use std::fmt::Debug;

use napi::bindgen_prelude::{TypeName, ValidateNapiValue};
use napi::{Either, bindgen_prelude::FromNapiValue, sys};

use rolldown_error::BuildDiagnostic;
use rolldown_utils::js_regex::HybridRegex;
use rolldown_utils::pattern_filter::StringOrRegex;

use super::js_regex::JsRegExp;

#[derive(Debug, Clone)]
pub struct BindingStringOrRegex(StringOrRegex);

#[cfg(test)]
impl BindingStringOrRegex {
  pub fn new(value: StringOrRegex) -> Self {
    Self(value)
  }
}

impl BindingStringOrRegex {
  pub fn inner(self) -> StringOrRegex {
    self.0
  }
}

type NapiStringOrRegex = Either<String, JsRegExp>;

impl TypeName for BindingStringOrRegex {
  fn type_name() -> &'static str {
    NapiStringOrRegex::type_name()
  }

  fn value_type() -> napi::ValueType {
    NapiStringOrRegex::value_type()
  }
}

impl ValidateNapiValue for BindingStringOrRegex {
  unsafe fn validate(
    env: sys::napi_env,
    napi_val: sys::napi_value,
  ) -> napi::Result<sys::napi_value> {
    unsafe { NapiStringOrRegex::validate(env, napi_val) }
  }
}

impl FromNapiValue for BindingStringOrRegex {
  unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
    unsafe {
      let value = NapiStringOrRegex::from_napi_value(env, napi_val)?;
      let value = match value {
        Either::A(inner) => StringOrRegex::String(inner),
        Either::B(inner) => {
          let reg = HybridRegex::with_flags(&inner.source, &inner.flags)?;
          StringOrRegex::Regex(reg)
        }
      };
      Ok(Self(value))
    }
  }
}

impl AsRef<StringOrRegex> for BindingStringOrRegex {
  fn as_ref(&self) -> &StringOrRegex {
    &self.0
  }
}

impl TryFrom<BindingStringOrRegex> for HybridRegex {
  type Error = BuildDiagnostic;

  fn try_from(value: BindingStringOrRegex) -> Result<Self, Self::Error> {
    Ok(match value.0 {
      StringOrRegex::String(value) => HybridRegex::new(&value)?,
      StringOrRegex::Regex(value) => value,
    })
  }
}

impl From<BindingStringOrRegex> for StringOrRegex {
  fn from(value: BindingStringOrRegex) -> Self {
    value.0
  }
}

pub fn bindingify_string_or_regex_array(items: Vec<BindingStringOrRegex>) -> Vec<StringOrRegex> {
  items.into_iter().map(|item| item.0).collect()
}
