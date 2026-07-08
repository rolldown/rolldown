pub mod native_error;

use napi::Either;
pub use native_error::NativeError;

#[napi_derive::napi(discriminant = "type", object_from_js = false)]
pub enum BindingError {
  JsError(napi::JsError),
  NativeError(NativeError),
}

impl BindingError {
  pub fn from_napi_error(error: &napi::Error) -> Self {
    // napi-rs cannot retain environment-bound exception references across
    // WASM worker boundaries. Native builds clone the shared ErrorRef, while
    // WASI preserves the status/reason message only.
    #[cfg(not(target_family = "wasm"))]
    {
      let error = error.try_clone().unwrap_or_else(|clone_error| clone_error);
      Self::JsError(napi::JsError::from(error))
    }
    #[cfg(target_family = "wasm")]
    {
      let error = napi::Error::new(error.status, error.reason.clone());
      Self::JsError(napi::JsError::from(error))
    }
  }
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
