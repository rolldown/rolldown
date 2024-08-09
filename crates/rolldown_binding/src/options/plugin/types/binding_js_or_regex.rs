use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Debug, Deserialize, Default)]
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
