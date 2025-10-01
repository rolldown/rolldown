pub mod native_error;

pub use native_error::NativeError;

#[napi_derive::napi(discriminant = "type", object_from_js = false)]
pub enum BindingError {
  JsError(napi::JsError),
  NativeError(NativeError),
}
