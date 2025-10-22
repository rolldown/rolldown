use std::fmt::Debug;

use napi::{
  Env, Error, Status, Unknown,
  bindgen_prelude::{
    FromNapiValue, Function, JsObjectValue, JsValue, Object, TypeName, ValidateNapiValue,
  },
  sys,
};

use rolldown_error::BuildDiagnostic;
use rolldown_utils::js_regex::HybridRegex;

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
    let js_object = Object::from_raw(env, napi_val);

    let env = Env::from(env);
    let global = env.get_global()?;
    let regexp_constructor = global.get_named_property::<Function<Unknown, ()>>("RegExp")?;

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
  type Error = BuildDiagnostic;

  fn try_from(value: JsRegExp) -> Result<Self, Self::Error> {
    Ok(HybridRegex::with_flags(&value.source, &value.flags)?)
  }
}
