#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingGeneratedCodeOptions {
  pub symbols: Option<bool>,
  pub preset: Option<String>, // "es5" | "es2015"
}
