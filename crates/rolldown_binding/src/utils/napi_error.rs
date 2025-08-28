pub fn resolve_error(specifier: &str, err: anyhow::Error) -> napi::Error {
  if err.downcast_ref::<napi::Error>().is_some() {
    let mut new_err =
      napi::Error::from_reason(format!("Errored while resolving {specifier:?} in `this.resolve`."));
    new_err.set_cause(err.downcast::<napi::Error>().unwrap());
    new_err
  } else {
    napi::Error::from_reason(format!(
      "Errored while resolving {specifier:?} in `this.resolve`. Got {err:?}."
    ))
  }
}

pub fn load_error(specifier: &str, err: anyhow::Error) -> napi::Error {
  if err.downcast_ref::<napi::Error>().is_some() {
    let mut new_err =
      napi::Error::from_reason(format!("Errored while load {specifier:?} in `this.load`."));
    new_err.set_cause(err.downcast::<napi::Error>().unwrap());
    new_err
  } else {
    napi::Error::from_reason(format!(
      "Errored while load {specifier:?} in `this.load`. Got {err:?}."
    ))
  }
}
