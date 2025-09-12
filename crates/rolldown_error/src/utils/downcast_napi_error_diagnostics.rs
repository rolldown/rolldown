use crate::BuildDiagnostic;

pub fn downcast_napi_error_diagnostics(err: anyhow::Error) -> anyhow::Result<BuildDiagnostic> {
  #[cfg(feature = "napi")]
  {
    err.downcast::<napi::Error>().map(BuildDiagnostic::napi_error)
  }
  #[cfg(not(feature = "napi"))]
  {
    Err(err)
  }
}
