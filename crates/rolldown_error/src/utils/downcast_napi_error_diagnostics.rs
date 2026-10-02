use crate::BuildDiagnostic;

pub fn downcast_napi_error_diagnostics(err: anyhow::Error) -> anyhow::Result<BuildDiagnostic> {
  // First try to downcast to BuildDiagnostic itself
  if err.is::<BuildDiagnostic>() {
    return err.downcast::<BuildDiagnostic>();
  }

  #[cfg(feature = "napi")]
  {
    if let Some(error) = err.chain().find_map(|cause| cause.downcast_ref::<napi::Error>()) {
      let error = error.try_clone().unwrap_or_else(|clone_error| clone_error);
      return Ok(BuildDiagnostic::napi_error(error));
    }
    Err(err)
  }
  #[cfg(not(feature = "napi"))]
  {
    Err(err)
  }
}

#[cfg(all(test, feature = "napi"))]
mod tests {
  use std::{fmt, sync::Arc};

  use super::downcast_napi_error_diagnostics;

  #[derive(Debug)]
  struct RetainedError(Arc<anyhow::Error>);

  impl fmt::Display for RetainedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      self.0.fmt(f)
    }
  }

  impl std::error::Error for RetainedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
      Some(self.0.root_cause())
    }
  }

  #[test]
  fn preserves_nested_napi_error_from_replayable_error() {
    let retained = anyhow::Error::new(napi::Error::from_reason("nested napi error"))
      .context("plugin close failed");
    let diagnostic =
      downcast_napi_error_diagnostics(anyhow::Error::new(RetainedError(Arc::new(retained))))
        .expect("nested napi error should be converted");
    let error = diagnostic.downcast_napi_error().expect("diagnostic should retain napi error");
    assert_eq!(error.reason, "nested napi error");
  }
}
