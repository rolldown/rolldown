use std::fmt::Debug;

pub fn resolve_error(specifier: &str, err: impl Debug) -> napi::Error {
  napi::Error::from_reason(format!(
    "Errored while resolving {specifier:?} in `this.resolve`. Got {err:?}."
  ))
}

pub fn load_error(specifier: &str, err: impl Debug) -> napi::Error {
  napi::Error::from_reason(format!("Errored while load {specifier:?} in `this.load`. Got {err:?}."))
}
