use std::fmt::Debug;

use napi::{
  bindgen_prelude::{FromNapiValue, Function, TypeName, ValidateNapiValue},
  sys, Either, Env, Error, JsObject, JsUnknown, NapiValue, Status,
};

use rolldown_utils::js_regex::HybridRegex;
use rolldown_utils::pattern_filter::StringOrRegex;

#[derive(Debug, Default, Clone)]
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

#[derive(Debug, Clone)]
pub struct BindingStringOrRegex(pub Either<String, JsRegExp>);

impl FromNapiValue for BindingStringOrRegex {
  unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
    Ok(Self(Either::from_napi_value(env, napi_val)?))
  }
}

impl TryFrom<BindingStringOrRegex> for HybridRegex {
  type Error = anyhow::Error;

  fn try_from(value: BindingStringOrRegex) -> Result<Self, Self::Error> {
    match value.0 {
      Either::A(value) => HybridRegex::new(&value),
      Either::B(value) => HybridRegex::try_from(value),
    }
  }
}

impl TryFrom<JsRegExp> for HybridRegex {
  type Error = anyhow::Error;

  fn try_from(value: JsRegExp) -> Result<Self, Self::Error> {
    HybridRegex::with_flags(&value.source, &value.flags)
  }
}

impl TryFrom<BindingStringOrRegex> for StringOrRegex {
  type Error = anyhow::Error;

  fn try_from(value: BindingStringOrRegex) -> Result<Self, Self::Error> {
    let ret = match value.0 {
      Either::A(inner) => StringOrRegex::String(inner),
      Either::B(inner) => {
        let reg = HybridRegex::with_flags(&inner.source, &inner.flags)?;
        StringOrRegex::Regex(reg)
      }
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
