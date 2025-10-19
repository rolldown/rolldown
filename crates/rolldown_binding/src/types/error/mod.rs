pub mod native_error;

use napi::Either;
pub use native_error::NativeError;

#[napi_derive::napi(discriminant = "type", object_from_js = false)]
pub enum BindingError {
  JsError(napi::JsError),
  NativeError(NativeError),
}

#[napi_derive::napi(object, object_from_js = false)]
pub struct BindingErrors {
  pub errors: Vec<BindingError>,
  pub is_binding_errors: bool,
}

impl BindingErrors {
  pub fn new(errors: Vec<BindingError>) -> Self {
    Self { errors, is_binding_errors: true }
  }
}

pub type BindingResult<T> = Either<BindingErrors, T>;
