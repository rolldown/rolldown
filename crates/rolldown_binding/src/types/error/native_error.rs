/// Error emitted from native side, it only contains kind and message, no stack trace.
// TODO: hyf0 do we want to rust stack trace?
#[napi_derive::napi(object)]
pub struct NativeError {
  pub kind: String,
  pub message: String,
}
