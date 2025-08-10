pub fn resolve_error(specifier: &str, err: anyhow::Error) -> napi::Error {
  let mut new_err =
    napi::Error::from_reason(format!("Errored while resolving {specifier:?} in `this.resolve`."));
  if let Ok(cause) = err.downcast::<napi::Error>() {
    new_err.set_cause(cause);
  }
  new_err
}

pub fn load_error(specifier: &str, err: anyhow::Error) -> napi::Error {
  let mut new_err =
    napi::Error::from_reason(format!("Errored while load {specifier:?} in `this.load`."));
  if let Ok(cause) = err.downcast::<napi::Error>() {
    new_err.set_cause(cause);
  }
  new_err
}
