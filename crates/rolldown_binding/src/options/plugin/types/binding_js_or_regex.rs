use std::fmt::Debug;

use napi::{
  bindgen_prelude::{FromNapiValue, Function, TypeName, ValidateNapiValue},
  sys, Env, Error, JsObject, JsUnknown, NapiValue, Status,
};

use rolldown_utils::js_regex::HybridRegex;
use rolldown_utils::pattern_filter::StringOrRegex;
use serde::Deserialize;

#[derive(Debug, Deserialize, Default, Clone)]
pub struct JsRegExp {
  pub source: String,
  pub flags: String,
}

impl ValidateNapiValue for JsRegExp {}

impl TypeName for JsRegExp {
  fn type_name() -> &'static str {
    "RegExp"
  }

  fn value_type() -> napi::ValueType {
    napi::ValueType::Object
  }
}

impl FromNapiValue for JsRegExp {
  unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
    let js_object = unsafe { JsObject::from_raw_unchecked(env, napi_val) };

    let env = Env::from(env);
    let global = env.get_global()?;
    let regexp_constructor = global.get_named_property::<Function<JsUnknown, ()>>("RegExp")?;

    if js_object.instanceof(regexp_constructor)? {
      let source = js_object.get_named_property::<String>("source")?;
      let flags = js_object.get_named_property::<String>("flags")?;

      Ok(JsRegExp { source, flags })
    } else {
      Err(Error::new(Status::ObjectExpected, "Expect a RegExp object"))
    }
  }
}

impl TryFrom<JsRegExp> for HybridRegex {
  type Error = anyhow::Error;

  fn try_from(value: JsRegExp) -> Result<Self, Self::Error> {
    HybridRegex::with_flags(&value.source, &value.flags)
  }
}

#[napi_derive::napi(object)]
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
/// For String, value is the string content, flag is the `None`
/// For Regex, value is the regular expression, flag is the `Some()`.
/// Make sure put a `Some("")` in flag even there is no flag in regexp.
pub struct BindingStringOrRegex {
  pub value: String,
  /// There is a more compact way to represent this, `Option<u8>` with bitflags, but it will be hard
  /// to use(in js side), since construct a `JsRegex` is not used frequently. Optimize it when it is needed.
  pub flag: Option<String>,
}

impl TryFrom<BindingStringOrRegex> for StringOrRegex {
  type Error = anyhow::Error;

  fn try_from(value: BindingStringOrRegex) -> Result<Self, Self::Error> {
    let ret = if let Some(flag) = value.flag {
      let reg = HybridRegex::with_flags(&value.value, &flag)?;
      StringOrRegex::Regex(reg)
    } else {
      StringOrRegex::String(value.value)
    };
    Ok(ret)
  }
}

pub fn bindingify_string_or_regex_array(
  items: Vec<BindingStringOrRegex>,
) -> anyhow::Result<Vec<StringOrRegex>> {
  let mut ret = Vec::with_capacity(items.len());
  for i in items {
    ret.push(i.try_into()?);
  }
  Ok(ret)
}
