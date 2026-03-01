use crate::BuildDiagnostic;

pub fn downcast_napi_error_diagnostics(err: anyhow::Error) -> anyhow::Result<BuildDiagnostic> {
  // First try to downcast to BuildDiagnostic itself
  if err.is::<BuildDiagnostic>() {
    return err.downcast::<BuildDiagnostic>();
  }

  #[cfg(feature = "napi")]
  {
    // Check if napi::Error is anywhere in the error chain (handles the case where it is wrapped
    // in context, e.g. `CausedPlugin → napi::Error`). Try to clone it first since
    // `downcast_ref` returns a borrow tied to `err`.
    let maybe_cloned = err.downcast_ref::<napi::Error>().and_then(|e| e.try_clone().ok());
    if let Some(cloned) = maybe_cloned {
      return Ok(BuildDiagnostic::napi_error(cloned));
    }
    // Fall back to direct downcast (works when napi::Error is at the root and try_clone failed)
    err.downcast::<napi::Error>().map(BuildDiagnostic::napi_error)
  }
  #[cfg(not(feature = "napi"))]
  {
    Err(err)
  }
}
