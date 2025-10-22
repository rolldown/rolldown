use rolldown_error::BuildDiagnostic;

pub fn resolve_error(specifier: &str, err: BuildDiagnostic) -> napi::Error {
  match err.downcast_napi_error() {
    Ok(napi_error) => {
      let mut new_err = napi::Error::from_reason(format!(
        "Errored while resolving {specifier:?} in `this.resolve`."
      ));
      #[cfg(not(target_family = "wasm"))]
      {
        new_err.set_cause(napi_error.try_clone().unwrap_or_else(|e| e));
      }
      #[cfg(target_family = "wasm")]
      {
        new_err.set_cause(napi::Error::new(napi_error.status, napi_error.reason.clone()));
      }
      new_err
    }
    Err(_) => napi::Error::from_reason(format!(
      "Errored while resolving {specifier:?} in `this.resolve`. Got {err:?}."
    )),
  }
}

pub fn load_error(specifier: &str, err: BuildDiagnostic) -> napi::Error {
  match err.downcast_napi_error() {
    Ok(napi_error) => {
      let mut new_err =
        napi::Error::from_reason(format!("Errored while load {specifier:?} in `this.load`."));
      #[cfg(not(target_family = "wasm"))]
      {
        new_err.set_cause(napi_error.try_clone().unwrap_or_else(|e| e));
      }
      #[cfg(target_family = "wasm")]
      {
        new_err.set_cause(napi::Error::new(napi_error.status, napi_error.reason.clone()));
      }
      new_err
    }
    Err(_) => napi::Error::from_reason(format!(
      "Errored while load {specifier:?} in `this.load`. Got {err:?}."
    )),
  }
}
